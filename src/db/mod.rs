use rusqlite::{Connection, Transaction};
use std::path::Path;

use crate::error::{MyceliumError, Result};
use crate::models::*;

mod migrations;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        conn.execute_batch("PRAGMA journal_mode = WAL")?;
        conn.execute_batch("PRAGMA synchronous = NORMAL")?;
        
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

    fn migrate(&mut self) -> Result<()> {
        migrations::run_migrations(&mut self.conn)
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
    pub fn create_epic(&mut self, title: &str, description: Option<&str>) -> Result<Epic> {
        let now = chrono::Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO epics (title, description, status, created_at, updated_at) 
             VALUES (?1, ?2, 'open', ?3, ?3)",
            (title, description, now),
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_epic(id)
            .map(|e| e.ok_or_else(|| MyceliumError::NotFound { 
                entity: "epic".to_string(), 
                id: id.to_string() 
            }))?
    }

    pub fn get_epic(&self, id: i64) -> Result<Option<Epic>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, created_at, updated_at FROM epics WHERE id = ?1"
        )?;
        
        let epic = stmt.query_row([id], |row| {
            Ok(Epic {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().unwrap(),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap().with_timezone(&chrono::Local),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap().with_timezone(&chrono::Local),
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
            "SELECT id, title, description, status, created_at, updated_at FROM epics ORDER BY created_at DESC"
        )?;
        
        let epics = stmt.query_map([], |row| {
            Ok(Epic {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().unwrap(),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap().with_timezone(&chrono::Local),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap().with_timezone(&chrono::Local),
            })
        })?;

        epics.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn list_epics_with_summary(&self) -> Result<Vec<epic::EpicSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                e.id, e.title, e.description, e.status, e.created_at, e.updated_at,
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
                    status: row.get::<_, String>(3)?.parse().unwrap(),
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .unwrap().with_timezone(&chrono::Local),
                    updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                        .unwrap().with_timezone(&chrono::Local),
                },
                total_tasks: row.get(6)?,
                open_tasks: row.get(7)?,
                closed_tasks: row.get(8)?,
            })
        })?;

        summaries.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn update_epic(&mut self, id: i64, title: Option<&str>, description: Option<&str>, status: Option<Status>) -> Result<Epic> {
        let now = chrono::Local::now().to_rfc3339();
        
        if let Some(title) = title {
            self.conn.execute("UPDATE epics SET title = ?1, updated_at = ?2 WHERE id = ?3", (title, &now, id))?;
        }
        if let Some(description) = description {
            self.conn.execute("UPDATE epics SET description = ?1, updated_at = ?2 WHERE id = ?3", (description, &now, id))?;
        }
        if let Some(status) = status {
            self.conn.execute("UPDATE epics SET status = ?1, updated_at = ?2 WHERE id = ?3", (status.to_string(), &now, id))?;
        }
        
        self.get_epic(id)
            .map(|e| e.ok_or_else(|| MyceliumError::NotFound { 
                entity: "epic".to_string(), 
                id: id.to_string() 
            }))?
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
    ) -> Result<Task> {
        let now = chrono::Local::now().to_rfc3339();
        let due_date_str = due_date.map(|d| d.to_string());
        
        self.conn.execute(
            "INSERT INTO tasks (title, description, status, priority, epic_id, assignee_id, due_date, tags, created_at, updated_at) 
             VALUES (?1, ?2, 'open', ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            (title, description, priority.to_string(), epic_id, assignee_id, due_date_str, tags, now),
        )?;
        
        let id = self.conn.last_insert_rowid();
        self.get_task(id)
            .map(|t| t.ok_or_else(|| MyceliumError::NotFound { 
                entity: "task".to_string(), 
                id: id.to_string() 
            }))?
    }

    pub fn get_task(&self, id: i64) -> Result<Option<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, status, priority, epic_id, assignee_id, due_date, tags, created_at, updated_at 
             FROM tasks WHERE id = ?1"
        )?;
        
        let task = stmt.query_row([id], |row| {
            let due_date: Option<String> = row.get(7)?;
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().unwrap(),
                priority: row.get::<_, String>(4)?.parse().unwrap(),
                epic_id: row.get(5)?,
                assignee_id: row.get(6)?,
                due_date: due_date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                tags: row.get(8)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                    .unwrap().with_timezone(&chrono::Local),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                    .unwrap().with_timezone(&chrono::Local),
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
            "SELECT id, title, description, status, priority, epic_id, assignee_id, due_date, tags, created_at, updated_at 
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
                status: row.get::<_, String>(3)?.parse().unwrap(),
                priority: row.get::<_, String>(4)?.parse().unwrap(),
                epic_id: row.get(5)?,
                assignee_id: row.get(6)?,
                due_date: due_date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                tags: row.get(8)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                    .unwrap().with_timezone(&chrono::Local),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                    .unwrap().with_timezone(&chrono::Local),
            })
        })?;

        let mut result: Vec<Task> = tasks.collect::<std::result::Result<Vec<Task>, rusqlite::Error>>().map_err(|e: rusqlite::Error| -> crate::error::MyceliumError { e.into() })?;

        if blocked_only {
            result.retain(|t| {
                self.get_blocking_tasks(t.id).map(|v| !v.is_empty()).unwrap_or(false)
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
    ) -> Result<Task> {
        let now = chrono::Local::now().to_rfc3339();
        
        if let Some(title) = title {
            self.conn.execute("UPDATE tasks SET title = ?1, updated_at = ?2 WHERE id = ?3", (title, &now, id))?;
        }
        if let Some(description) = description {
            self.conn.execute("UPDATE tasks SET description = ?1, updated_at = ?2 WHERE id = ?3", (description, &now, id))?;
        }
        if let Some(status) = status {
            self.conn.execute("UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3", (status.to_string(), &now, id))?;
        }
        if let Some(priority) = priority {
            self.conn.execute("UPDATE tasks SET priority = ?1, updated_at = ?2 WHERE id = ?3", (priority.to_string(), &now, id))?;
        }
        if let Some(epic_id) = epic_id {
            self.conn.execute("UPDATE tasks SET epic_id = ?1, updated_at = ?2 WHERE id = ?3", (epic_id, &now, id))?;
        }
        if let Some(assignee_id) = assignee_id {
            self.conn.execute("UPDATE tasks SET assignee_id = ?1, updated_at = ?2 WHERE id = ?3", (assignee_id, &now, id))?;
        }
        if let Some(due_date) = due_date {
            let due_str = due_date.map(|d| d.to_string());
            self.conn.execute("UPDATE tasks SET due_date = ?1, updated_at = ?2 WHERE id = ?3", (due_str, &now, id))?;
        }
        if let Some(tags) = tags {
            self.conn.execute("UPDATE tasks SET tags = ?1, updated_at = ?2 WHERE id = ?3", (tags, &now, id))?;
        }
        
        self.get_task(id)
            .map(|t| t.ok_or_else(|| MyceliumError::NotFound { 
                entity: "task".to_string(), 
                id: id.to_string() 
            }))?
    }

    pub fn delete_task(&mut self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        Ok(())
    }

    // Assignee operations
    pub fn create_assignee(&mut self, name: &str, email: Option<&str>, github: Option<&str>) -> Result<Assignee> {
        self.conn.execute(
            "INSERT INTO assignees (name, email, github_username, created_at) 
             VALUES (?1, ?2, ?3, ?4)",
            (name, email, github, chrono::Local::now().to_rfc3339()),
        )?;
        
        let id = self.conn.last_insert_rowid();
        self.get_assignee(id)
            .map(|a| a.ok_or_else(|| MyceliumError::NotFound { 
                entity: "assignee".to_string(), 
                id: id.to_string() 
            }))?
    }

    pub fn get_assignee(&self, id: i64) -> Result<Option<Assignee>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, email, github_username, created_at FROM assignees WHERE id = ?1"
        )?;
        
        let assignee = stmt.query_row([id], |row| {
            Ok(Assignee {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                github_username: row.get(3)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap().with_timezone(&chrono::Local),
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
            "SELECT id, name, email, github_username, created_at FROM assignees ORDER BY name"
        )?;
        
        let assignees = stmt.query_map([], |row| {
            Ok(Assignee {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                github_username: row.get(3)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap().with_timezone(&chrono::Local),
            })
        })?;

        assignees.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.into())
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
             ORDER BY a.name"
        )?;
        
        let stats = stmt.query_map([], |row| {
            Ok(assignee::AssigneeWithStats {
                assignee: Assignee {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    email: row.get(2)?,
                    github_username: row.get(3)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .unwrap().with_timezone(&chrono::Local),
                },
                total_tasks: row.get(5)?,
                open_tasks: row.get(6)?,
            })
        })?;

        stats.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn delete_assignee(&mut self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM assignees WHERE id = ?1", [id])?;
        Ok(())
    }

    // Dependency operations
    pub fn add_dependency(&mut self, task_id: i64, depends_on_task_id: i64) -> Result<()> {
        // Check for circular dependency
        if self.would_create_cycle(task_id, depends_on_task_id)? {
            return Err(MyceliumError::CircularDependency(
                format!("Task {} already depends on task {} (directly or indirectly)", depends_on_task_id, task_id)
            ));
        }
        
        self.conn.execute(
            "INSERT INTO dependencies (task_id, depends_on_task_id, created_at) 
             VALUES (?1, ?2, ?3)",
            (task_id, depends_on_task_id, chrono::Local::now().to_rfc3339()),
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
        let mut stmt = self.conn.prepare(
            "SELECT depends_on_task_id FROM dependencies WHERE task_id = ?1"
        )?;
        
        let ids = stmt.query_map([task_id], |row| row.get(0))?;
        ids.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn get_blocked_tasks(&self, task_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT task_id FROM dependencies WHERE depends_on_task_id = ?1"
        )?;
        
        let ids = stmt.query_map([task_id], |row| row.get(0))?;
        ids.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.into())
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

    // External reference operations
    pub fn add_external_ref(&mut self, task_id: i64, ref_type: ExternalRefType, reference: &str) -> Result<ExternalRef> {
        self.conn.execute(
            "INSERT INTO external_refs (task_id, ref_type, reference, created_at) 
             VALUES (?1, ?2, ?3, ?4)",
            (task_id, ref_type.to_string(), reference, chrono::Local::now().to_rfc3339()),
        )?;
        
        let id = self.conn.last_insert_rowid();
        self.get_external_ref(id)
            .map(|r| r.ok_or_else(|| MyceliumError::NotFound { 
                entity: "external_ref".to_string(), 
                id: id.to_string() 
            }))?
    }

    pub fn get_external_ref(&self, id: i64) -> Result<Option<ExternalRef>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, ref_type, reference, created_at FROM external_refs WHERE id = ?1"
        )?;
        
        let ext_ref = stmt.query_row([id], |row| {
            Ok(ExternalRef {
                id: row.get(0)?,
                task_id: row.get(1)?,
                ref_type: row.get::<_, String>(2)?.parse().unwrap(),
                reference: row.get(3)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap().with_timezone(&chrono::Local),
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
             ORDER BY created_at"
        )?;
        
        let refs = stmt.query_map([task_id], |row| {
            Ok(ExternalRef {
                id: row.get(0)?,
                task_id: row.get(1)?,
                ref_type: row.get::<_, String>(2)?.parse().unwrap(),
                reference: row.get(3)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap().with_timezone(&chrono::Local),
            })
        })?;

        refs.collect::<std::result::Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn remove_external_ref(&mut self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM external_refs WHERE id = ?1", [id])?;
        Ok(())
    }

    // Summary operations
    pub fn get_summary(&self) -> Result<Summary> {
        let total_epics: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM epics", [], |row| row.get(0)
        )?;
        
        let open_epics: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM epics WHERE status = 'open'", [], |row| row.get(0)
        )?;
        
        let total_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks", [], |row| row.get(0)
        )?;
        
        let open_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'open'", [], |row| row.get(0)
        )?;
        
        let overdue_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'open' AND due_date < ?1",
            [chrono::Local::now().naive_local().date().to_string()],
            |row| row.get(0)
        )?;
        
        let blocked_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT d.task_id) FROM dependencies d 
             JOIN tasks t ON t.id = d.depends_on_task_id 
             WHERE t.status = 'open'", [], |row| row.get(0)
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


