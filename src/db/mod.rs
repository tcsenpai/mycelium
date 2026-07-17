use rusqlite::{Connection, Transaction};
use std::path::Path;

use crate::error::{MyceliumError, Result};
use crate::models::*;

mod migrations;

/// Parse a status string from the database, returning a rusqlite error on failure.
fn parse_status(s: &str) -> std::result::Result<Status, rusqlite::Error> {
    s.parse().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

/// Parse a priority string from the database, returning a rusqlite error on failure.
fn parse_priority(s: &str) -> std::result::Result<Priority, rusqlite::Error> {
    s.parse().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

/// Parse an RFC3339 timestamp from the database into a local DateTime.
fn parse_timestamp(
    s: &str,
) -> std::result::Result<chrono::DateTime<chrono::Local>, rusqlite::Error> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Local))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
}

/// Parse an ExternalRefType string from the database.
fn parse_ref_type(s: &str) -> std::result::Result<ExternalRefType, rusqlite::Error> {
    s.parse().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        conn.execute_batch("PRAGMA journal_mode = WAL")?;
        conn.execute_batch("PRAGMA synchronous = NORMAL")?;
        // Each `myc` invocation opens its own connection. Under WAL, SQLite still
        // serializes writers with a single write lock; a second writer that
        // arrives while the lock is held gets SQLITE_BUSY *immediately* unless a
        // busy timeout is set. Without this, rapid/concurrent `task create`
        // invocations (and the per-startup ensure_schema DDL) can fail or drop
        // writes. 5s is ample for a local CLI.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        let mut db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;

        let mut db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn migrate(&mut self) -> Result<()> {
        migrations::run_migrations(&mut self.conn)
    }

    pub fn get_conn(&self) -> &Connection {
        &self.conn
    }

    pub fn transaction<T, F>(&mut self, f: F) -> Result<T>
    where
        F: FnOnce(&Transaction) -> Result<T>,
    {
        let tx = self.conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    // Epic operations
    pub fn create_epic(
        &mut self,
        title: &str,
        description: Option<&str>,
        notes: Option<&str>,
        user_info: Option<&str>,
    ) -> Result<Epic> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO epics (title, description, status, notes, user_info, created_at, updated_at)
             VALUES (?1, ?2, 'open', ?3, ?4, ?5, ?5)",
            (title, description, notes, user_info, now),
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_epic(id).map(|e| {
            e.ok_or_else(|| MyceliumError::NotFound {
                entity: "epic".to_string(),
                id: id.to_string(),
            })
        })?
    }

    pub fn get_epic(&self, id: i64) -> Result<Option<Epic>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, notes, user_info, agent_questions, created_at, updated_at FROM epics WHERE id = ?1"
        )?;

        let epic = stmt.query_row([id], |row| {
            Ok(Epic {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: parse_status(&row.get::<_, String>(3)?)?,
                notes: row.get(4)?,
                user_info: row.get(5)?,
                agent_questions: row.get(6)?,
                created_at: parse_timestamp(&row.get::<_, String>(7)?)?,
                updated_at: parse_timestamp(&row.get::<_, String>(8)?)?,
            })
        });

        match epic {
            Ok(e) => Ok(Some(e)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_epics(&self) -> Result<Vec<Epic>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, notes, user_info, agent_questions, created_at, updated_at FROM epics ORDER BY created_at DESC"
        )?;

        let epics = stmt.query_map([], |row| {
            Ok(Epic {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: parse_status(&row.get::<_, String>(3)?)?,
                notes: row.get(4)?,
                user_info: row.get(5)?,
                agent_questions: row.get(6)?,
                created_at: parse_timestamp(&row.get::<_, String>(7)?)?,
                updated_at: parse_timestamp(&row.get::<_, String>(8)?)?,
            })
        })?;

        epics
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn list_epics_with_summary(&self) -> Result<Vec<epic::EpicSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                e.id, e.title, e.description, e.status, e.notes, e.user_info, e.agent_questions, e.created_at, e.updated_at,
                COUNT(t.id) as total_tasks,
                SUM(CASE WHEN t.status = 'open' THEN 1 ELSE 0 END) as open_tasks,
                SUM(CASE WHEN t.status = 'closed' THEN 1 ELSE 0 END) as closed_tasks
             FROM epics e
             LEFT JOIN tasks t ON t.epic_id = e.id
             GROUP BY e.id
             ORDER BY e.created_at DESC"
        )?;

        let summaries = stmt.query_map([], |row| {
            Ok(epic::EpicSummary {
                epic: Epic {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    status: parse_status(&row.get::<_, String>(3)?)?,
                    notes: row.get(4)?,
                    user_info: row.get(5)?,
                    agent_questions: row.get(6)?,
                    created_at: parse_timestamp(&row.get::<_, String>(7)?)?,
                    updated_at: parse_timestamp(&row.get::<_, String>(8)?)?,
                },
                total_tasks: row.get(9)?,
                open_tasks: row.get(10)?,
                closed_tasks: row.get(11)?,
            })
        })?;

        summaries
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn update_epic(
        &mut self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        status: Option<Status>,
        notes: Option<Option<&str>>,
        user_info: Option<Option<&str>>,
        agent_questions: Option<Option<&str>>,
    ) -> Result<Epic> {
        let now = chrono::Local::now().to_rfc3339();

        if let Some(title) = title {
            self.conn.execute(
                "UPDATE epics SET title = ?1, updated_at = ?2 WHERE id = ?3",
                (title, &now, id),
            )?;
        }
        if let Some(description) = description {
            self.conn.execute(
                "UPDATE epics SET description = ?1, updated_at = ?2 WHERE id = ?3",
                (description, &now, id),
            )?;
        }
        if let Some(status) = status {
            self.conn.execute(
                "UPDATE epics SET status = ?1, updated_at = ?2 WHERE id = ?3",
                (status.to_string(), &now, id),
            )?;
        }
        if let Some(notes) = notes {
            self.conn.execute(
                "UPDATE epics SET notes = ?1, updated_at = ?2 WHERE id = ?3",
                (notes, &now, id),
            )?;
        }
        if let Some(user_info) = user_info {
            self.conn.execute(
                "UPDATE epics SET user_info = ?1, updated_at = ?2 WHERE id = ?3",
                (user_info, &now, id),
            )?;
        }
        if let Some(agent_questions) = agent_questions {
            self.conn.execute(
                "UPDATE epics SET agent_questions = ?1, updated_at = ?2 WHERE id = ?3",
                (agent_questions, &now, id),
            )?;
        }

        self.get_epic(id).map(|e| {
            e.ok_or_else(|| MyceliumError::NotFound {
                entity: "epic".to_string(),
                id: id.to_string(),
            })
        })?
    }

    pub fn delete_epic(&mut self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM epics WHERE id = ?1", [id])?;
        Ok(())
    }

    // Task operations
    pub fn create_task(
        &mut self,
        title: &str,
        description: Option<&str>,
        epic_id: Option<i64>,
        priority: Priority,
        assignee_id: Option<i64>,
        due_date: Option<chrono::NaiveDate>,
        tags: Option<&str>,
        notes: Option<&str>,
        user_info: Option<&str>,
    ) -> Result<Task> {
        let now = chrono::Local::now().to_rfc3339();
        let due_date_str = due_date.map(|d| d.to_string());

        self.conn.execute(
            "INSERT INTO tasks (title, description, status, priority, epic_id, assignee_id, due_date, tags, notes, user_info, created_at, updated_at)
             VALUES (?1, ?2, 'open', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            (title, description, priority.to_string(), epic_id, assignee_id, due_date_str, tags, notes, user_info, now),
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_task(id).map(|t| {
            t.ok_or_else(|| MyceliumError::NotFound {
                entity: "task".to_string(),
                id: id.to_string(),
            })
        })?
    }

    pub fn get_task(&self, id: i64) -> Result<Option<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, priority, epic_id, assignee_id, due_date, tags, notes, user_info, agent_questions, created_at, updated_at
             FROM tasks WHERE id = ?1"
        )?;

        let task = stmt.query_row([id], |row| {
            let due_date: Option<String> = row.get(7)?;
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: parse_status(&row.get::<_, String>(3)?)?,
                priority: parse_priority(&row.get::<_, String>(4)?)?,
                epic_id: row.get(5)?,
                assignee_id: row.get(6)?,
                due_date: due_date
                    .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                tags: row.get(8)?,
                notes: row.get(9)?,
                user_info: row.get(10)?,
                agent_questions: row.get(11)?,
                created_at: parse_timestamp(&row.get::<_, String>(12)?)?,
                updated_at: parse_timestamp(&row.get::<_, String>(13)?)?,
            })
        });

        match task {
            Ok(t) => Ok(Some(t)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_tasks(
        &self,
        epic_id: Option<i64>,
        status: Option<Status>,
        priority: Option<Priority>,
        assignee_id: Option<i64>,
        blocked_only: bool,
        overdue_only: bool,
        tag: Option<&str>,
    ) -> Result<Vec<Task>> {
        let mut conditions = vec!["1=1"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(eid) = epic_id {
            conditions.push("epic_id = ?");
            params.push(Box::new(eid));
        }
        if let Some(s) = status {
            conditions.push("status = ?");
            params.push(Box::new(s.to_string()));
        }
        if let Some(p) = priority {
            conditions.push("priority = ?");
            params.push(Box::new(p.to_string()));
        }
        if let Some(aid) = assignee_id {
            conditions.push("assignee_id = ?");
            params.push(Box::new(aid));
        }
        if overdue_only {
            let today = chrono::Local::now().naive_local().date().to_string();
            conditions.push("due_date < ? AND status = 'open'");
            params.push(Box::new(today));
        }
        if let Some(t) = tag {
            conditions.push("tags LIKE ?");
            params.push(Box::new(format!("%{}%", t)));
        }

        let sql = format!(
            "SELECT id, title, description, status, priority, epic_id, assignee_id, due_date, tags, notes, user_info, agent_questions, created_at, updated_at
             FROM tasks
             WHERE {}
             ORDER BY
                CASE priority
                    WHEN 'critical' THEN 1
                    WHEN 'high' THEN 2
                    WHEN 'medium' THEN 3
                    ELSE 4
                END,
                created_at DESC",
            conditions.join(" AND ")
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let tasks = stmt.query_map(&*param_refs, |row| {
            let due_date: Option<String> = row.get(7)?;
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: parse_status(&row.get::<_, String>(3)?)?,
                priority: parse_priority(&row.get::<_, String>(4)?)?,
                epic_id: row.get(5)?,
                assignee_id: row.get(6)?,
                due_date: due_date
                    .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                tags: row.get(8)?,
                notes: row.get(9)?,
                user_info: row.get(10)?,
                agent_questions: row.get(11)?,
                created_at: parse_timestamp(&row.get::<_, String>(12)?)?,
                updated_at: parse_timestamp(&row.get::<_, String>(13)?)?,
            })
        })?;

        let mut result: Vec<Task> = tasks
            .collect::<std::result::Result<Vec<Task>, rusqlite::Error>>()
            .map_err(|e: rusqlite::Error| -> crate::error::MyceliumError { e.into() })?;

        if blocked_only {
            result.retain(|t| {
                self.get_open_blockers(t.id)
                    .map(|v| !v.is_empty())
                    .unwrap_or(false)
            });
        }

        Ok(result)
    }

    pub fn update_task(
        &mut self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        status: Option<Status>,
        priority: Option<Priority>,
        epic_id: Option<Option<i64>>,
        assignee_id: Option<Option<i64>>,
        due_date: Option<Option<chrono::NaiveDate>>,
        tags: Option<Option<&str>>,
        notes: Option<Option<&str>>,
        user_info: Option<Option<&str>>,
        agent_questions: Option<Option<&str>>,
    ) -> Result<Task> {
        let now = chrono::Local::now().to_rfc3339();

        if let Some(title) = title {
            self.conn.execute(
                "UPDATE tasks SET title = ?1, updated_at = ?2 WHERE id = ?3",
                (title, &now, id),
            )?;
        }
        if let Some(description) = description {
            self.conn.execute(
                "UPDATE tasks SET description = ?1, updated_at = ?2 WHERE id = ?3",
                (description, &now, id),
            )?;
        }
        if let Some(status) = status {
            self.conn.execute(
                "UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3",
                (status.to_string(), &now, id),
            )?;
        }
        if let Some(priority) = priority {
            self.conn.execute(
                "UPDATE tasks SET priority = ?1, updated_at = ?2 WHERE id = ?3",
                (priority.to_string(), &now, id),
            )?;
        }
        if let Some(epic_id) = epic_id {
            self.conn.execute(
                "UPDATE tasks SET epic_id = ?1, updated_at = ?2 WHERE id = ?3",
                (epic_id, &now, id),
            )?;
        }
        if let Some(assignee_id) = assignee_id {
            self.conn.execute(
                "UPDATE tasks SET assignee_id = ?1, updated_at = ?2 WHERE id = ?3",
                (assignee_id, &now, id),
            )?;
        }
        if let Some(due_date) = due_date {
            let due_str = due_date.map(|d| d.to_string());
            self.conn.execute(
                "UPDATE tasks SET due_date = ?1, updated_at = ?2 WHERE id = ?3",
                (due_str, &now, id),
            )?;
        }
        if let Some(tags) = tags {
            self.conn.execute(
                "UPDATE tasks SET tags = ?1, updated_at = ?2 WHERE id = ?3",
                (tags, &now, id),
            )?;
        }
        if let Some(notes) = notes {
            self.conn.execute(
                "UPDATE tasks SET notes = ?1, updated_at = ?2 WHERE id = ?3",
                (notes, &now, id),
            )?;
        }
        if let Some(user_info) = user_info {
            self.conn.execute(
                "UPDATE tasks SET user_info = ?1, updated_at = ?2 WHERE id = ?3",
                (user_info, &now, id),
            )?;
        }
        if let Some(agent_questions) = agent_questions {
            self.conn.execute(
                "UPDATE tasks SET agent_questions = ?1, updated_at = ?2 WHERE id = ?3",
                (agent_questions, &now, id),
            )?;
        }

        self.get_task(id).map(|t| {
            t.ok_or_else(|| MyceliumError::NotFound {
                entity: "task".to_string(),
                id: id.to_string(),
            })
        })?
    }

    pub fn delete_task(&mut self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        Ok(())
    }

    // Assignee operations
    pub fn create_assignee(
        &mut self,
        name: &str,
        email: Option<&str>,
        github: Option<&str>,
    ) -> Result<Assignee> {
        self.conn.execute(
            "INSERT INTO assignees (name, email, github_username, created_at) 
             VALUES (?1, ?2, ?3, ?4)",
            (name, email, github, chrono::Local::now().to_rfc3339()),
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_assignee(id).map(|a| {
            a.ok_or_else(|| MyceliumError::NotFound {
                entity: "assignee".to_string(),
                id: id.to_string(),
            })
        })?
    }

    pub fn get_assignee(&self, id: i64) -> Result<Option<Assignee>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, email, github_username, created_at FROM assignees WHERE id = ?1",
        )?;

        let assignee = stmt.query_row([id], |row| {
            Ok(Assignee {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                github_username: row.get(3)?,
                created_at: parse_timestamp(&row.get::<_, String>(4)?)?,
            })
        });

        match assignee {
            Ok(a) => Ok(Some(a)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_assignees(&self) -> Result<Vec<Assignee>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, email, github_username, created_at FROM assignees ORDER BY name",
        )?;

        let assignees = stmt.query_map([], |row| {
            Ok(Assignee {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                github_username: row.get(3)?,
                created_at: parse_timestamp(&row.get::<_, String>(4)?)?,
            })
        })?;

        assignees
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn list_assignees_with_stats(&self) -> Result<Vec<assignee::AssigneeWithStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                a.id, a.name, a.email, a.github_username, a.created_at,
                COUNT(t.id) as total_tasks,
                SUM(CASE WHEN t.status = 'open' THEN 1 ELSE 0 END) as open_tasks
             FROM assignees a
             LEFT JOIN tasks t ON t.assignee_id = a.id
             GROUP BY a.id
             ORDER BY a.name",
        )?;

        let stats = stmt.query_map([], |row| {
            Ok(assignee::AssigneeWithStats {
                assignee: Assignee {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    email: row.get(2)?,
                    github_username: row.get(3)?,
                    created_at: parse_timestamp(&row.get::<_, String>(4)?)?,
                },
                total_tasks: row.get(5)?,
                open_tasks: row.get(6)?,
            })
        })?;

        stats
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn delete_assignee(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM assignees WHERE id = ?1", [id])?;
        Ok(())
    }

    // Dependency operations
    pub fn add_dependency(&mut self, task_id: i64, depends_on_task_id: i64) -> Result<()> {
        // Check for circular dependency
        if self.would_create_cycle(task_id, depends_on_task_id)? {
            return Err(MyceliumError::CircularDependency(format!(
                "Task {} already depends on task {} (directly or indirectly)",
                depends_on_task_id, task_id
            )));
        }

        self.conn.execute(
            "INSERT INTO dependencies (task_id, depends_on_task_id, created_at) 
             VALUES (?1, ?2, ?3)",
            (
                task_id,
                depends_on_task_id,
                chrono::Local::now().to_rfc3339(),
            ),
        )?;
        Ok(())
    }

    pub fn remove_dependency(&mut self, task_id: i64, depends_on_task_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM dependencies WHERE task_id = ?1 AND depends_on_task_id = ?2",
            (task_id, depends_on_task_id),
        )?;
        Ok(())
    }

    pub fn get_blocking_tasks(&self, task_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT depends_on_task_id FROM dependencies WHERE task_id = ?1")?;

        let ids = stmt.query_map([task_id], |row| row.get(0))?;
        ids.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn get_blocked_tasks(&self, task_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT task_id FROM dependencies WHERE depends_on_task_id = ?1")?;

        let ids = stmt.query_map([task_id], |row| row.get(0))?;
        ids.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn get_all_dependencies(&self, task_id: i64) -> Result<dependency::DependencyChain> {
        let blocking = self.get_blocking_tasks(task_id)?;
        let blocked = self.get_blocked_tasks(task_id)?;

        // Get transitive dependencies
        let mut all_deps = vec![];
        let mut to_check = blocking.clone();
        let mut visited = std::collections::HashSet::new();

        while let Some(check_id) = to_check.pop() {
            if visited.insert(check_id) {
                all_deps.push(check_id);
                let deps = self.get_blocking_tasks(check_id)?;
                to_check.extend(deps);
            }
        }

        Ok(dependency::DependencyChain {
            task_id,
            blocked_by: blocking,
            blocks: blocked,
            all_dependencies: all_deps,
        })
    }

    fn would_create_cycle(&self, task_id: i64, depends_on_task_id: i64) -> Result<bool> {
        // Check if depends_on_task_id already depends on task_id
        let all_deps = self.get_all_dependencies(depends_on_task_id)?;
        Ok(all_deps.all_dependencies.contains(&task_id) || depends_on_task_id == task_id)
    }

    pub fn get_open_blockers(&self, task_id: i64) -> Result<Vec<Task>> {
        let blocking_ids = self.get_blocking_tasks(task_id)?;
        let mut open_blockers = vec![];

        for id in blocking_ids {
            if let Some(task) = self.get_task(id)? {
                if task.status == Status::Open {
                    open_blockers.push(task);
                }
            }
        }

        Ok(open_blockers)
    }

    /// Get dependencies for multiple tasks at once
    /// Returns a map of task_id -> (blocked_by_ids, blocks_ids)
    pub fn get_dependencies_for_tasks(
        &self,
        task_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, (Vec<i64>, Vec<i64>)>> {
        use std::collections::HashMap;

        if task_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        // Get all blocking relationships for these tasks
        let sql = format!(
            "SELECT task_id, depends_on_task_id FROM dependencies 
             WHERE task_id IN ({}) OR depends_on_task_id IN ({})",
            placeholders, placeholders
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = task_ids
            .iter()
            .chain(task_ids.iter())
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        let mut result: HashMap<i64, (Vec<i64>, Vec<i64>)> = HashMap::new();

        // Initialize empty entries for all task IDs
        for &id in task_ids {
            result.insert(id, (vec![], vec![]));
        }

        let rows = stmt.query_map(&*params, |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })?;

        for row in rows {
            let (task_id, depends_on_id) = row?;

            // task_id is blocked by depends_on_id
            if task_ids.contains(&task_id) {
                result.entry(task_id).or_default().0.push(depends_on_id);
            }

            // depends_on_id blocks task_id
            if task_ids.contains(&depends_on_id) {
                result.entry(depends_on_id).or_default().1.push(task_id);
            }
        }

        Ok(result)
    }

    // Task note operations
    pub fn add_task_note(&mut self, task_id: i64, content: &str) -> Result<TaskNote> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO task_notes (task_id, content, created_at) VALUES (?1, ?2, ?3)",
            (task_id, content, &now),
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_task_note(id).map(|n| {
            n.ok_or_else(|| MyceliumError::NotFound {
                entity: "task_note".to_string(),
                id: id.to_string(),
            })
        })?
    }

    pub fn get_task_note(&self, id: i64) -> Result<Option<TaskNote>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, task_id, content, created_at FROM task_notes WHERE id = ?1")?;

        let note = stmt.query_row([id], |row| {
            Ok(TaskNote {
                id: row.get(0)?,
                task_id: row.get(1)?,
                content: row.get(2)?,
                created_at: parse_timestamp(&row.get::<_, String>(3)?)?,
            })
        });

        match note {
            Ok(n) => Ok(Some(n)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_task_notes(&self, task_id: i64) -> Result<Vec<TaskNote>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, content, created_at FROM task_notes WHERE task_id = ?1 ORDER BY created_at DESC"
        )?;

        let notes = stmt.query_map([task_id], |row| {
            Ok(TaskNote {
                id: row.get(0)?,
                task_id: row.get(1)?,
                content: row.get(2)?,
                created_at: parse_timestamp(&row.get::<_, String>(3)?)?,
            })
        })?;

        notes
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn delete_task_note(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM task_notes WHERE id = ?1", [id])?;
        Ok(())
    }

    // Epic note operations
    pub fn add_epic_note(&mut self, epic_id: i64, content: &str) -> Result<EpicNote> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO epic_notes (epic_id, content, created_at) VALUES (?1, ?2, ?3)",
            (epic_id, content, &now),
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_epic_note(id).map(|n| {
            n.ok_or_else(|| MyceliumError::NotFound {
                entity: "epic_note".to_string(),
                id: id.to_string(),
            })
        })?
    }

    pub fn get_epic_note(&self, id: i64) -> Result<Option<EpicNote>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, epic_id, content, created_at FROM epic_notes WHERE id = ?1")?;

        let note = stmt.query_row([id], |row| {
            Ok(EpicNote {
                id: row.get(0)?,
                epic_id: row.get(1)?,
                content: row.get(2)?,
                created_at: parse_timestamp(&row.get::<_, String>(3)?)?,
            })
        });

        match note {
            Ok(n) => Ok(Some(n)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_epic_notes(&self, epic_id: i64) -> Result<Vec<EpicNote>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, epic_id, content, created_at FROM epic_notes WHERE epic_id = ?1 ORDER BY created_at DESC"
        )?;

        let notes = stmt.query_map([epic_id], |row| {
            Ok(EpicNote {
                id: row.get(0)?,
                epic_id: row.get(1)?,
                content: row.get(2)?,
                created_at: parse_timestamp(&row.get::<_, String>(3)?)?,
            })
        })?;

        notes
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    // Batch operations
    pub fn batch_close_tasks(&mut self, task_ids: &[i64], force: bool) -> Result<Vec<Task>> {
        let mut closed_tasks = Vec::new();
        let now = chrono::Local::now().to_rfc3339();

        for &id in task_ids {
            // Check for blockers unless force is true
            if !force {
                let blockers = self.get_open_blockers(id)?;
                if !blockers.is_empty() {
                    continue; // Skip this task if blocked
                }
            }

            // Update task status
            self.conn.execute(
                "UPDATE tasks SET status = 'closed', updated_at = ?1 WHERE id = ?2 AND status = 'open'",
                (&now, id),
            )?;

            if let Ok(Some(task)) = self.get_task(id) {
                if task.status == Status::Closed {
                    closed_tasks.push(task);
                }
            }
        }

        Ok(closed_tasks)
    }

    pub fn batch_add_tag(&mut self, task_ids: &[i64], tag: &str) -> Result<Vec<Task>> {
        let mut updated_tasks = Vec::new();
        let now = chrono::Local::now().to_rfc3339();

        for &id in task_ids {
            if let Ok(Some(task)) = self.get_task(id) {
                let current_tags = task.tags.unwrap_or_default();
                let new_tags = if current_tags.is_empty() {
                    tag.to_string()
                } else if current_tags.contains(tag) {
                    current_tags // Tag already exists
                } else {
                    format!("{}, {}", current_tags, tag)
                };

                self.conn.execute(
                    "UPDATE tasks SET tags = ?1, updated_at = ?2 WHERE id = ?3",
                    (&new_tags, &now, id),
                )?;

                if let Ok(Some(updated)) = self.get_task(id) {
                    updated_tasks.push(updated);
                }
            }
        }

        Ok(updated_tasks)
    }

    pub fn batch_move_to_epic(
        &mut self,
        task_ids: &[i64],
        epic_id: Option<i64>,
    ) -> Result<Vec<Task>> {
        let mut updated_tasks = Vec::new();
        let now = chrono::Local::now().to_rfc3339();

        // Verify epic exists if specified
        if let Some(eid) = epic_id {
            if self.get_epic(eid)?.is_none() {
                return Err(MyceliumError::NotFound {
                    entity: "epic".to_string(),
                    id: eid.to_string(),
                });
            }
        }

        for &id in task_ids {
            self.conn.execute(
                "UPDATE tasks SET epic_id = ?1, updated_at = ?2 WHERE id = ?3",
                (epic_id, &now, id),
            )?;

            if let Ok(Some(updated)) = self.get_task(id) {
                updated_tasks.push(updated);
            }
        }

        Ok(updated_tasks)
    }

    // Task cloning
    pub fn clone_task(&mut self, task_id: i64, new_title: Option<&str>) -> Result<Task> {
        let original = self
            .get_task(task_id)?
            .ok_or_else(|| MyceliumError::NotFound {
                entity: "task".to_string(),
                id: task_id.to_string(),
            })?;

        let default_title = format!("{} (copy)", original.title);
        let title = new_title.unwrap_or(&default_title);
        let now = chrono::Local::now();
        let now_str = now.to_rfc3339();

        self.conn.execute(
            "INSERT INTO tasks (title, description, status, priority, epic_id, assignee_id, due_date, tags, notes, user_info, created_at, updated_at)
             VALUES (?1, ?2, 'open', ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, ?9)",
            (
                title,
                original.description,
                original.priority.to_string(),
                original.epic_id,
                original.due_date.map(|d| d.to_string()),
                original.tags,
                original.notes,
                original.user_info,
                &now_str,
            ),
        )?;

        let new_id = self.conn.last_insert_rowid();

        // Clone external references
        let refs = self.list_external_refs(task_id)?;
        for ext_ref in refs {
            self.add_external_ref(new_id, ext_ref.ref_type, &ext_ref.reference)?;
        }

        self.get_task(new_id).map(|t| {
            t.ok_or_else(|| MyceliumError::NotFound {
                entity: "task".to_string(),
                id: new_id.to_string(),
            })
        })?
    }

    // External reference operations
    pub fn add_external_ref(
        &mut self,
        task_id: i64,
        ref_type: ExternalRefType,
        reference: &str,
    ) -> Result<ExternalRef> {
        self.conn.execute(
            "INSERT INTO external_refs (task_id, ref_type, reference, created_at) 
             VALUES (?1, ?2, ?3, ?4)",
            (
                task_id,
                ref_type.to_string(),
                reference,
                chrono::Local::now().to_rfc3339(),
            ),
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_external_ref(id).map(|r| {
            r.ok_or_else(|| MyceliumError::NotFound {
                entity: "external_ref".to_string(),
                id: id.to_string(),
            })
        })?
    }

    pub fn get_external_ref(&self, id: i64) -> Result<Option<ExternalRef>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, ref_type, reference, created_at FROM external_refs WHERE id = ?1",
        )?;

        let ext_ref = stmt.query_row([id], |row| {
            Ok(ExternalRef {
                id: row.get(0)?,
                task_id: row.get(1)?,
                ref_type: parse_ref_type(&row.get::<_, String>(2)?)?,
                reference: row.get(3)?,
                created_at: parse_timestamp(&row.get::<_, String>(4)?)?,
            })
        });

        match ext_ref {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_external_refs(&self, task_id: i64) -> Result<Vec<ExternalRef>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, ref_type, reference, created_at 
             FROM external_refs 
             WHERE task_id = ?1
             ORDER BY created_at",
        )?;

        let refs = stmt.query_map([task_id], |row| {
            Ok(ExternalRef {
                id: row.get(0)?,
                task_id: row.get(1)?,
                ref_type: parse_ref_type(&row.get::<_, String>(2)?)?,
                reference: row.get(3)?,
                created_at: parse_timestamp(&row.get::<_, String>(4)?)?,
            })
        })?;

        refs.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn remove_external_ref(&mut self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM external_refs WHERE id = ?1", [id])?;
        Ok(())
    }

    // Summary operations
    pub fn get_summary(&self) -> Result<Summary> {
        let total_epics: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM epics", [], |row| row.get(0))?;

        let open_epics: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM epics WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;

        let total_tasks: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))?;

        let open_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;

        let overdue_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'open' AND due_date < ?1",
            [chrono::Local::now().naive_local().date().to_string()],
            |row| row.get(0),
        )?;

        let blocked_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT d.task_id) FROM dependencies d 
             JOIN tasks t ON t.id = d.depends_on_task_id 
             WHERE t.status = 'open'",
            [],
            |row| row.get(0),
        )?;

        Ok(Summary {
            total_epics,
            open_epics,
            closed_epics: total_epics - open_epics,
            total_tasks,
            open_tasks,
            closed_tasks: total_tasks - open_tasks,
            overdue_tasks,
            blocked_tasks,
        })
    }

    // Linear sync operations
    pub fn create_linear_sync(
        &mut self,
        local_task_id: i64,
        linear_issue_id: &str,
        linear_issue_identifier: Option<&str>,
        last_local_hash: &str,
        last_remote_hash: &str,
    ) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO linear_sync (local_task_id, linear_issue_id, linear_issue_identifier, last_synced_at, last_local_hash, last_remote_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (local_task_id, linear_issue_id, linear_issue_identifier, &now, last_local_hash, last_remote_hash),
        )?;
        Ok(())
    }

    pub fn get_linear_sync_by_task(&self, task_id: i64) -> Result<Option<LinearSyncEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, local_task_id, linear_issue_id, linear_issue_identifier, last_synced_at, last_local_hash, last_remote_hash, sync_direction
             FROM linear_sync WHERE local_task_id = ?1"
        )?;
        let entry = stmt.query_row([task_id], |row| {
            Ok(LinearSyncEntry {
                id: row.get(0)?,
                local_task_id: row.get(1)?,
                linear_issue_id: row.get(2)?,
                linear_issue_identifier: row.get(3)?,
                last_synced_at: row.get(4)?,
                last_local_hash: row.get(5)?,
                last_remote_hash: row.get(6)?,
                sync_direction: row.get(7)?,
            })
        });
        match entry {
            Ok(e) => Ok(Some(e)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_linear_sync_by_issue(
        &self,
        linear_issue_id: &str,
    ) -> Result<Option<LinearSyncEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, local_task_id, linear_issue_id, linear_issue_identifier, last_synced_at, last_local_hash, last_remote_hash, sync_direction
             FROM linear_sync WHERE linear_issue_id = ?1"
        )?;
        let entry = stmt.query_row([linear_issue_id], |row| {
            Ok(LinearSyncEntry {
                id: row.get(0)?,
                local_task_id: row.get(1)?,
                linear_issue_id: row.get(2)?,
                linear_issue_identifier: row.get(3)?,
                last_synced_at: row.get(4)?,
                last_local_hash: row.get(5)?,
                last_remote_hash: row.get(6)?,
                sync_direction: row.get(7)?,
            })
        });
        match entry {
            Ok(e) => Ok(Some(e)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn update_linear_sync(
        &mut self,
        local_task_id: i64,
        last_local_hash: &str,
        last_remote_hash: &str,
    ) -> Result<()> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "UPDATE linear_sync SET last_synced_at = ?1, last_local_hash = ?2, last_remote_hash = ?3 WHERE local_task_id = ?4",
            (&now, last_local_hash, last_remote_hash, local_task_id),
        )?;
        Ok(())
    }

    pub fn delete_all_linear_sync(&mut self) -> Result<()> {
        self.conn.execute("DELETE FROM linear_sync", [])?;
        Ok(())
    }

    pub fn count_linear_synced(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM linear_sync", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn get_last_sync_time(&self) -> Result<Option<String>> {
        let result: std::result::Result<String, rusqlite::Error> =
            self.conn
                .query_row("SELECT MAX(last_synced_at) FROM linear_sync", [], |row| {
                    row.get(0)
                });
        match result {
            Ok(t) => Ok(Some(t)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(rusqlite::Error::InvalidColumnType(_, _, _)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all tasks (no filtering). Used by sync.
    pub fn list_all_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, priority, epic_id, assignee_id, due_date, tags, notes, user_info, agent_questions, created_at, updated_at
             FROM tasks ORDER BY id"
        )?;
        let tasks = stmt.query_map([], |row| {
            let due_date: Option<String> = row.get(7)?;
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: parse_status(&row.get::<_, String>(3)?)?,
                priority: parse_priority(&row.get::<_, String>(4)?)?,
                epic_id: row.get(5)?,
                assignee_id: row.get(6)?,
                due_date: due_date
                    .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                tags: row.get(8)?,
                notes: row.get(9)?,
                user_info: row.get(10)?,
                agent_questions: row.get(11)?,
                created_at: parse_timestamp(&row.get::<_, String>(12)?)?,
                updated_at: parse_timestamp(&row.get::<_, String>(13)?)?,
            })
        })?;
        tasks
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// List tasks that have no epic assigned.
    pub fn list_orphan_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, priority, epic_id, assignee_id, due_date, tags, notes, user_info, agent_questions, created_at, updated_at
             FROM tasks WHERE epic_id IS NULL ORDER BY id"
        )?;
        let tasks = stmt.query_map([], |row| {
            let due_date: Option<String> = row.get(7)?;
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: parse_status(&row.get::<_, String>(3)?)?,
                priority: parse_priority(&row.get::<_, String>(4)?)?,
                epic_id: row.get(5)?,
                assignee_id: row.get(6)?,
                due_date: due_date
                    .and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                tags: row.get(8)?,
                notes: row.get(9)?,
                user_info: row.get(10)?,
                agent_questions: row.get(11)?,
                created_at: parse_timestamp(&row.get::<_, String>(12)?)?,
                updated_at: parse_timestamp(&row.get::<_, String>(13)?)?,
            })
        })?;
        tasks
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Delete all tasks that have no epic assigned. Returns count deleted.
    pub fn delete_orphan_tasks(&mut self) -> Result<usize> {
        let count = self
            .conn
            .execute("DELETE FROM tasks WHERE epic_id IS NULL", [])?;
        Ok(count)
    }

    // Follow-up operations

    pub fn create_followup(&mut self, body: &str, title: Option<&str>) -> Result<Followup> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO followups (body, title, status, created_at) VALUES (?1, ?2, 'open', ?3)",
            (body, title, &now),
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_followup(id)?
            .ok_or_else(|| MyceliumError::NotFound {
                entity: "followup".to_string(),
                id: id.to_string(),
            })
    }

    pub fn get_followup(&self, id: i64) -> Result<Option<Followup>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, body, title, status, closure_reason, created_at, closed_at
             FROM followups WHERE id = ?1",
        )?;

        let row = stmt.query_row([id], |row| {
            let closed_at: Option<String> = row.get(6)?;
            Ok(Followup {
                id: row.get(0)?,
                body: row.get(1)?,
                title: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                closure_reason: row.get(4)?,
                created_at: parse_timestamp(&row.get::<_, String>(5)?)?,
                closed_at: closed_at.as_deref().map(parse_timestamp).transpose()?,
            })
        });

        match row {
            Ok(f) => Ok(Some(f)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List follow-ups. Filter semantics:
    /// - None / `Some("all")` → every row
    /// - `Some("active")` → open + in_progress
    /// - `Some("closed")` → done + wontfix
    /// - exact status string → that one bucket
    pub fn list_followups(&self, status: Option<&str>) -> Result<Vec<Followup>> {
        let (sql, want_filter): (&str, Option<String>) = match status {
            None | Some("all") => (
                "SELECT id, body, title, status, closure_reason, created_at, closed_at
                 FROM followups ORDER BY id",
                None,
            ),
            Some("active") => (
                "SELECT id, body, title, status, closure_reason, created_at, closed_at
                 FROM followups WHERE status IN ('open', 'in_progress') ORDER BY id",
                None,
            ),
            Some("closed") => (
                "SELECT id, body, title, status, closure_reason, created_at, closed_at
                 FROM followups WHERE status IN ('done', 'wontfix') ORDER BY id",
                None,
            ),
            Some(other) => {
                let parsed: FollowupStatus = other.parse()?;
                (
                    "SELECT id, body, title, status, closure_reason, created_at, closed_at
                     FROM followups WHERE status = ?1 ORDER BY id",
                    Some(parsed.as_str().to_string()),
                )
            }
        };

        let mut stmt = self.conn.prepare(sql)?;
        let map_row = |row: &rusqlite::Row| -> rusqlite::Result<Followup> {
            let closed_at: Option<String> = row.get(6)?;
            Ok(Followup {
                id: row.get(0)?,
                body: row.get(1)?,
                title: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                closure_reason: row.get(4)?,
                created_at: parse_timestamp(&row.get::<_, String>(5)?)?,
                closed_at: closed_at.as_deref().map(parse_timestamp).transpose()?,
            })
        };

        let rows = if let Some(s) = want_filter {
            stmt.query_map([s], map_row)?
                .collect::<std::result::Result<Vec<_>, _>>()
        } else {
            stmt.query_map([], map_row)?
                .collect::<std::result::Result<Vec<_>, _>>()
        };
        rows.map_err(|e| e.into())
    }

    /// Lowest-id active follow-up (open or in_progress). Agent loop primitive.
    pub fn next_followup(&self) -> Result<Option<Followup>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, body, title, status, closure_reason, created_at, closed_at
             FROM followups WHERE status IN ('open', 'in_progress') ORDER BY id LIMIT 1",
        )?;

        let row = stmt.query_row([], |row| {
            let closed_at: Option<String> = row.get(6)?;
            Ok(Followup {
                id: row.get(0)?,
                body: row.get(1)?,
                title: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                closure_reason: row.get(4)?,
                created_at: parse_timestamp(&row.get::<_, String>(5)?)?,
                closed_at: closed_at.as_deref().map(parse_timestamp).transpose()?,
            })
        });

        match row {
            Ok(f) => Ok(Some(f)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn update_followup_status(
        &mut self,
        id: i64,
        new_status: FollowupStatus,
        closure_reason: Option<&str>,
    ) -> Result<Followup> {
        let existing = self
            .get_followup(id)?
            .ok_or_else(|| MyceliumError::NotFound {
                entity: "followup".to_string(),
                id: id.to_string(),
            })?;

        let now = chrono::Local::now().to_rfc3339();
        let closed_at: Option<String> = if new_status.is_open() {
            None
        } else if existing.closed_at.is_some() && existing.status == new_status {
            existing.closed_at.map(|t| t.to_rfc3339())
        } else {
            Some(now)
        };

        let reason: Option<String> = match new_status {
            FollowupStatus::Wontfix => closure_reason.map(|s| s.to_string()),
            FollowupStatus::Done => closure_reason.map(|s| s.to_string()),
            _ => None,
        };

        self.conn.execute(
            "UPDATE followups SET status = ?1, closure_reason = ?2, closed_at = ?3 WHERE id = ?4",
            (
                new_status.as_str(),
                reason.as_deref(),
                closed_at.as_deref(),
                id,
            ),
        )?;

        self.get_followup(id)?
            .ok_or_else(|| MyceliumError::NotFound {
                entity: "followup".to_string(),
                id: id.to_string(),
            })
    }

    pub fn update_followup_body(
        &mut self,
        id: i64,
        body: Option<&str>,
        title: Option<&str>,
        clear_title: bool,
    ) -> Result<Followup> {
        let existing = self
            .get_followup(id)?
            .ok_or_else(|| MyceliumError::NotFound {
                entity: "followup".to_string(),
                id: id.to_string(),
            })?;

        let new_body = body.unwrap_or(&existing.body);
        let new_title: Option<String> = if clear_title {
            None
        } else if let Some(t) = title {
            Some(t.to_string())
        } else {
            existing.title.clone()
        };

        self.conn.execute(
            "UPDATE followups SET body = ?1, title = ?2 WHERE id = ?3",
            (new_body, new_title.as_deref(), id),
        )?;

        self.get_followup(id)?
            .ok_or_else(|| MyceliumError::NotFound {
                entity: "followup".to_string(),
                id: id.to_string(),
            })
    }

    pub fn delete_followup(&mut self, id: i64) -> Result<()> {
        let count = self
            .conn
            .execute("DELETE FROM followups WHERE id = ?1", [id])?;
        if count == 0 {
            return Err(MyceliumError::NotFound {
                entity: "followup".to_string(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    /// Count rows grouped by status. Returns zero for absent buckets.
    pub fn count_followups(&self) -> Result<FollowupCounts> {
        let mut stmt = self
            .conn
            .prepare("SELECT status, COUNT(*) FROM followups GROUP BY status")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        let mut counts = FollowupCounts::default();
        for r in rows {
            let (status, n) = r?;
            match status.as_str() {
                "open" => counts.open = n,
                "in_progress" => counts.in_progress = n,
                "done" => counts.done = n,
                "wontfix" => counts.wontfix = n,
                _ => {}
            }
        }
        Ok(counts)
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FollowupCounts {
    pub open: i64,
    pub in_progress: i64,
    pub done: i64,
    pub wontfix: i64,
}

impl FollowupCounts {
    pub fn active(&self) -> i64 {
        self.open + self.in_progress
    }
}

#[derive(Debug, Clone)]
pub struct LinearSyncEntry {
    pub id: i64,
    pub local_task_id: i64,
    pub linear_issue_id: String,
    pub linear_issue_identifier: Option<String>,
    pub last_synced_at: String,
    pub last_local_hash: String,
    pub last_remote_hash: String,
    pub sync_direction: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Summary {
    pub total_epics: i64,
    pub open_epics: i64,
    pub closed_epics: i64,
    pub total_tasks: i64,
    pub open_tasks: i64,
    pub closed_tasks: i64,
    pub overdue_tasks: i64,
    pub blocked_tasks: i64,
}
