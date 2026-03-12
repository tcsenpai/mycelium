use rusqlite::{Connection, params};
use chrono::Local;
use crate::models::*;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        conn.execute_batch("PRAGMA journal_mode = WAL")?;
        conn.execute_batch("PRAGMA synchronous = NORMAL")?;

        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), rusqlite::Error> {
        Self::init_schema(&self.conn)?;
        if !Self::has_column(&self.conn, "tasks", "tags")? {
            self.conn.execute("ALTER TABLE tasks ADD COLUMN tags TEXT", [])?;
        }
        Ok(())
    }

    fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, rusqlite::Error> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for row in rows {
            if row? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn init_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS epics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL DEFAULT 'open',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS assignees (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT,
                github_username TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT,
                status TEXT NOT NULL DEFAULT 'open',
                priority TEXT NOT NULL DEFAULT 'medium',
                epic_id INTEGER,
                assignee_id INTEGER,
                due_date TEXT,
                tags TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (epic_id) REFERENCES epics(id) ON DELETE SET NULL,
                FOREIGN KEY (assignee_id) REFERENCES assignees(id) ON DELETE SET NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                depends_on_task_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                FOREIGN KEY (depends_on_task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                UNIQUE(task_id, depends_on_task_id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_epic ON tasks(epic_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_assignee ON tasks(assignee_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_priority ON tasks(priority)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_dependencies_task ON dependencies(task_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_dependencies_depends_on ON dependencies(depends_on_task_id)",
            [],
        )?;

        Ok(())
    }

    // Dashboard stats
    pub fn get_dashboard_stats(&self) -> Result<DashboardStats, rusqlite::Error> {
        let total_epics: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM epics",
            [],
            |row| row.get(0),
        )?;

        let open_epics: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM epics WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;

        let total_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks",
            [],
            |row| row.get(0),
        )?;

        let open_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'open'",
            [],
            |row| row.get(0),
        )?;

        let overdue_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'open' AND due_date < ?",
            [Local::now().naive_local().date().to_string()],
            |row| row.get(0),
        )?;

        let blocked_tasks: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT d.task_id) FROM dependencies d 
             JOIN tasks t ON t.id = d.depends_on_task_id 
             WHERE t.status = 'open'",
            [],
            |row| row.get(0),
        )?;

        let high_priority_open: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'open' AND priority IN ('high', 'critical')",
            [],
            |row| row.get(0),
        )?;

        let completion_rate = if total_tasks > 0 {
            ((total_tasks - open_tasks) as f64 / total_tasks as f64) * 100.0
        } else {
            0.0
        };

        Ok(DashboardStats {
            total_epics,
            open_epics,
            closed_epics: total_epics - open_epics,
            total_tasks,
            open_tasks,
            closed_tasks: total_tasks - open_tasks,
            overdue_tasks,
            blocked_tasks,
            high_priority_open,
            completion_rate,
        })
    }

    // Tasks
    pub fn get_tasks(&self, filters: TaskFilters) -> Result<Vec<Task>, rusqlite::Error> {
        let mut sql = String::from(
            "SELECT 
                t.id, t.title, t.description, t.status, t.priority,
                t.epic_id, e.title as epic_title,
                t.assignee_id, a.name as assignee_name,
                t.due_date, t.tags, t.created_at, t.updated_at
             FROM tasks t
             LEFT JOIN epics e ON t.epic_id = e.id
             LEFT JOIN assignees a ON t.assignee_id = a.id
             WHERE 1=1"
        );

        if filters.epic_id.is_some() {
            sql.push_str(" AND t.epic_id = ?");
        }
        if filters.status.is_some() {
            sql.push_str(" AND t.status = ?");
        }
        if filters.priority.is_some() {
            sql.push_str(" AND t.priority = ?");
        }
        if filters.assignee_id.is_some() {
            sql.push_str(" AND t.assignee_id = ?");
        }
        if filters.tag.is_some() {
            sql.push_str(" AND t.tags LIKE ?");
        }
        if filters.overdue {
            sql.push_str(" AND t.status = 'open' AND t.due_date < ?");
        }

        sql.push_str(" ORDER BY 
            CASE t.priority 
                WHEN 'critical' THEN 1 
                WHEN 'high' THEN 2 
                WHEN 'medium' THEN 3 
                ELSE 4 
            END,
            t.created_at DESC"
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(id) = filters.epic_id {
            params.push(Box::new(id));
        }
        if let Some(status) = filters.status {
            params.push(Box::new(status.to_string()));
        }
        if let Some(priority) = filters.priority {
            params.push(Box::new(priority.to_string()));
        }
        if let Some(id) = filters.assignee_id {
            params.push(Box::new(id));
        }
        if let Some(tag) = filters.tag {
            params.push(Box::new(format!("%{}%", tag)));
        }
        if filters.overdue {
            params.push(Box::new(Local::now().naive_local().date().to_string()));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let tasks = stmt.query_map(&*param_refs, |row| {
            let due_date: Option<String> = row.get(9)?;
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                priority: row.get::<_, String>(4)?.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                epic_id: row.get(5)?,
                epic_title: row.get(6)?,
                assignee_id: row.get(7)?,
                assignee_name: row.get(8)?,
                due_date: due_date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                tags: row.get(10)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                    .unwrap().with_timezone(&Local),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(12)?)
                    .unwrap().with_timezone(&Local),
                blocked_by: Vec::new(),
                blocks: Vec::new(),
            })
        })?;

        let mut result = Vec::new();
        for task in tasks {
            let mut task = task?;
            task.blocked_by = self.get_open_blocker_ids(task.id)?;
            task.blocks = self.get_blocked_tasks(task.id)?;
            result.push(task);
        }

        // Filter by blocked status in memory
        if filters.blocked {
            result.retain(|t| !t.blocked_by.is_empty());
        }

        // Filter by search term
        if let Some(search) = filters.search {
            let search_lower = search.to_lowercase();
            result.retain(|t| {
                t.title.to_lowercase().contains(&search_lower) ||
                t.description.as_ref().map(|d| d.to_lowercase().contains(&search_lower)).unwrap_or(false)
            });
        }

        Ok(result)
    }

    pub fn get_task(&self, id: i64) -> Result<Option<Task>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                t.id, t.title, t.description, t.status, t.priority,
                t.epic_id, e.title as epic_title,
                t.assignee_id, a.name as assignee_name,
                t.due_date, t.tags, t.created_at, t.updated_at
             FROM tasks t
             LEFT JOIN epics e ON t.epic_id = e.id
             LEFT JOIN assignees a ON t.assignee_id = a.id
             WHERE t.id = ?1"
        )?;

        let task = stmt.query_row([id], |row| {
            let due_date: Option<String> = row.get(9)?;
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                priority: row.get::<_, String>(4)?.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                epic_id: row.get(5)?,
                epic_title: row.get(6)?,
                assignee_id: row.get(7)?,
                assignee_name: row.get(8)?,
                due_date: due_date.and_then(|d| chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok()),
                tags: row.get(10)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(11)?)
                    .unwrap().with_timezone(&Local),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(12)?)
                    .unwrap().with_timezone(&Local),
                blocked_by: self.get_open_blocker_ids(id)?,
                blocks: self.get_blocked_tasks(id)?,
            })
        });

        match task {
            Ok(task) => Ok(Some(task)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn create_task(&mut self, task: NewTask) -> Result<Task, rusqlite::Error> {
        let now = Local::now().to_rfc3339();
        let due_date = task.due_date;

        self.conn.execute(
            "INSERT INTO tasks (title, description, priority, epic_id, assignee_id, due_date, tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                task.title,
                task.description,
                task.priority.to_string(),
                task.epic_id,
                task.assignee_id,
                due_date,
                task.tags,
                now
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_task(id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_task(&mut self, id: i64, updates: TaskUpdate) -> Result<Task, rusqlite::Error> {
        let now = Local::now().to_rfc3339();

        if let Some(title) = updates.title {
            self.conn.execute("UPDATE tasks SET title = ?1, updated_at = ?2 WHERE id = ?3", params![title, now, id])?;
        }
        if let Some(description) = updates.description {
            self.conn.execute("UPDATE tasks SET description = ?1, updated_at = ?2 WHERE id = ?3", params![description, now, id])?;
        }
        if let Some(status) = updates.status {
            self.conn.execute("UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = ?3", params![status.to_string(), now, id])?;
        }
        if let Some(priority) = updates.priority {
            self.conn.execute("UPDATE tasks SET priority = ?1, updated_at = ?2 WHERE id = ?3", params![priority.to_string(), now, id])?;
        }
        if let Some(epic_id) = updates.epic_id {
            self.conn.execute("UPDATE tasks SET epic_id = ?1, updated_at = ?2 WHERE id = ?3", params![epic_id, now, id])?;
        }
        if let Some(assignee_id) = updates.assignee_id {
            self.conn.execute("UPDATE tasks SET assignee_id = ?1, updated_at = ?2 WHERE id = ?3", params![assignee_id, now, id])?;
        }
        if let Some(due_date) = updates.due_date {
            self.conn.execute("UPDATE tasks SET due_date = ?1, updated_at = ?2 WHERE id = ?3", params![due_date, now, id])?;
        }
        if let Some(tags) = updates.tags {
            self.conn.execute("UPDATE tasks SET tags = ?1, updated_at = ?2 WHERE id = ?3", params![tags, now, id])?;
        }

        self.get_task(id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn delete_task(&mut self, id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn close_task(&mut self, id: i64) -> Result<Task, rusqlite::Error> {
        self.update_task(id, TaskUpdate {
            status: Some(Status::Closed),
            ..Default::default()
        })
    }

    pub fn reopen_task(&mut self, id: i64) -> Result<Task, rusqlite::Error> {
        self.update_task(id, TaskUpdate {
            status: Some(Status::Open),
            ..Default::default()
        })
    }

    // Epics
    pub fn get_epics(&self) -> Result<Vec<Epic>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                e.id, e.title, e.description, e.status, e.created_at, e.updated_at,
                COUNT(t.id) as total_tasks,
                COALESCE(SUM(CASE WHEN t.status = 'open' THEN 1 ELSE 0 END), 0) as open_tasks
             FROM epics e
             LEFT JOIN tasks t ON t.epic_id = e.id
             GROUP BY e.id
             ORDER BY e.created_at DESC"
        )?;

        let epics = stmt.query_map([], |row| {
            Ok(Epic {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                status: row.get::<_, String>(3)?.parse().map_err(|e| rusqlite::Error::InvalidParameterName(e))?,
                total_tasks: row.get(6)?,
                open_tasks: row.get(7)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap().with_timezone(&Local),
                updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap().with_timezone(&Local),
            })
        })?;

        epics.collect()
    }

    pub fn get_epic(&self, id: i64) -> Result<Option<Epic>, rusqlite::Error> {
        let epics = self.get_epics()?;
        Ok(epics.into_iter().find(|e| e.id == id))
    }

    pub fn create_epic(&mut self, epic: NewEpic) -> Result<Epic, rusqlite::Error> {
        let now = Local::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO epics (title, description, status, created_at, updated_at)
             VALUES (?1, ?2, 'open', ?3, ?3)",
            params![epic.title, epic.description, now],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_epic(id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn update_epic(&mut self, id: i64, updates: EpicUpdate) -> Result<Epic, rusqlite::Error> {
        let now = Local::now().to_rfc3339();

        if let Some(title) = updates.title {
            self.conn.execute("UPDATE epics SET title = ?1, updated_at = ?2 WHERE id = ?3", params![title, now, id])?;
        }
        if let Some(description) = updates.description {
            self.conn.execute("UPDATE epics SET description = ?1, updated_at = ?2 WHERE id = ?3", params![description, now, id])?;
        }
        if let Some(status) = updates.status {
            self.conn.execute("UPDATE epics SET status = ?1, updated_at = ?2 WHERE id = ?3", params![status.to_string(), now, id])?;
        }

        self.get_epic(id)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    pub fn delete_epic(&mut self, id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute("DELETE FROM epics WHERE id = ?1", [id])?;
        Ok(())
    }

    // Assignees
    pub fn get_assignees(&self) -> Result<Vec<Assignee>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                a.id, a.name, a.email, a.github_username,
                COUNT(t.id) as total_tasks,
                COALESCE(SUM(CASE WHEN t.status = 'open' THEN 1 ELSE 0 END), 0) as open_tasks
             FROM assignees a
             LEFT JOIN tasks t ON t.assignee_id = a.id
             GROUP BY a.id
             ORDER BY a.name"
        )?;

        let assignees: Result<Vec<Assignee>, rusqlite::Error> = stmt.query_map([], |row| {
            Ok(Assignee {
                id: row.get(0)?,
                name: row.get(1)?,
                email: row.get(2)?,
                github_username: row.get(3)?,
                total_tasks: row.get(4)?,
                open_tasks: row.get(5)?,
            })
        })?.collect();
        assignees
    }

    pub fn create_assignee(&mut self, assignee: NewAssignee) -> Result<Assignee, rusqlite::Error> {
        let now = Local::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO assignees (name, email, github_username, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![assignee.name, assignee.email, assignee.github_username, now],
        )?;

        let id = self.conn.last_insert_rowid();
        let assignees = self.get_assignees()?;
        assignees
            .into_iter()
            .find(|a| a.id == id)
            .ok_or(rusqlite::Error::QueryReturnedNoRows)
    }

    // Dependencies
    pub fn get_blocking_tasks(&self, task_id: i64) -> Result<Vec<i64>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT depends_on_task_id FROM dependencies WHERE task_id = ?"
        )?;

        let ids: Result<Vec<i64>, rusqlite::Error> = stmt.query_map([task_id], |row| row.get(0))?.collect();
        ids
    }

    pub fn get_blocked_tasks(&self, task_id: i64) -> Result<Vec<i64>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT task_id FROM dependencies WHERE depends_on_task_id = ?"
        )?;

        let ids: Result<Vec<i64>, rusqlite::Error> = stmt.query_map([task_id], |row| row.get(0))?.collect();
        ids
    }

    pub fn get_open_blocker_ids(&self, task_id: i64) -> Result<Vec<i64>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT d.depends_on_task_id
             FROM dependencies d
             JOIN tasks t ON t.id = d.depends_on_task_id
             WHERE d.task_id = ?1 AND t.status = 'open'"
        )?;

        let ids: Result<Vec<i64>, rusqlite::Error> = stmt.query_map([task_id], |row| row.get(0))?.collect();
        ids
    }

    pub fn get_open_blockers(&self, task_id: i64) -> Result<Vec<Task>, rusqlite::Error> {
        let blocker_ids = self.get_open_blocker_ids(task_id)?;
        let mut blockers = Vec::new();

        for blocker_id in blocker_ids {
            if let Some(task) = self.get_task(blocker_id)? {
                blockers.push(task);
            }
        }

        Ok(blockers)
    }

    pub fn add_dependency(&mut self, task_id: i64, depends_on: i64) -> Result<(), rusqlite::Error> {
        if self.would_create_cycle(task_id, depends_on)? {
            return Err(rusqlite::Error::InvalidParameterName(
                "Dependency would create a cycle".to_string(),
            ));
        }

        let now = Local::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO dependencies (task_id, depends_on_task_id, created_at)
             VALUES (?1, ?2, ?3)",
            params![task_id, depends_on, now],
        )?;
        Ok(())
    }

    pub fn remove_dependency(&mut self, task_id: i64, depends_on: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM dependencies WHERE task_id = ?1 AND depends_on_task_id = ?2",
            params![task_id, depends_on],
        )?;
        Ok(())
    }

    pub fn get_dependencies(&self, task_id: i64) -> Result<DependencyChain, rusqlite::Error> {
        let blocked_by = self.get_blocking_tasks(task_id)?;
        let blocks = self.get_blocked_tasks(task_id)?;

        // Get transitive dependencies
        let mut all = Vec::new();
        let mut to_check = blocked_by.clone();
        let mut visited = std::collections::HashSet::new();

        while let Some(check_id) = to_check.pop() {
            if visited.insert(check_id) {
                all.push(check_id);
                let deps = self.get_blocking_tasks(check_id)?;
                to_check.extend(deps);
            }
        }

        Ok(DependencyChain {
            task_id,
            blocked_by,
            blocks,
            all_dependencies: all,
        })
    }

    fn would_create_cycle(&self, task_id: i64, depends_on: i64) -> Result<bool, rusqlite::Error> {
        if task_id == depends_on {
            return Ok(true);
        }

        let deps = self.get_dependencies(depends_on)?;
        Ok(deps.all_dependencies.contains(&task_id))
    }

    // Search
    pub fn search_tasks(&self, query: &str) -> Result<Vec<Task>, rusqlite::Error> {
        self.get_tasks(TaskFilters {
            search: Some(query.to_string()),
            ..Default::default()
        })
    }

    // Tags
    pub fn get_all_tags(&self) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT tags FROM tasks WHERE tags IS NOT NULL")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

        let mut all_tags = std::collections::HashSet::new();
        for row in rows {
            let tags = row?;
            for tag in tags.split(',') {
                all_tags.insert(tag.trim().to_string());
            }
        }

        let mut result: Vec<String> = all_tags.into_iter().collect();
        result.sort();
        Ok(result)
    }
}
