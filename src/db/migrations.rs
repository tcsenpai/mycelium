use rusqlite::Connection;
use crate::error::Result;

const CURRENT_VERSION: i32 = 1;

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    create_migrations_table(conn)?;
    
    let version = get_current_version(conn)?;
    
    if version < 1 {
        migrate_v1(conn)?;
        set_version(conn, 1)?;
    }
    
    Ok(())
}

fn create_migrations_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        )",
        [],
    )?;
    Ok(())
}

fn get_current_version(conn: &Connection) -> Result<i32> {
    // Check if migrations table exists
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='_migrations')",
        [],
        |row| row.get(0),
    )?;
    
    if !exists {
        return Ok(0);
    }
    
    let result: std::result::Result<Option<i32>, rusqlite::Error> = conn.query_row(
        "SELECT MAX(version) FROM _migrations",
        [],
        |row| row.get(0),
    );
    
    match result {
        Ok(v) => Ok(v.unwrap_or(0)),
        Err(rusqlite::Error::InvalidColumnType(_, _, _)) => Ok(0),
        Err(e) => Err(e.into()),
    }
}

fn set_version(conn: &Connection, version: i32) -> Result<()> {
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO _migrations (version, applied_at) VALUES (?1, ?2)",
        (version, now),
    )?;
    Ok(())
}

fn migrate_v1(conn: &Connection) -> Result<()> {
    // Epics table
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
    
    // Assignees table
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
    
    // Tasks table
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
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (epic_id) REFERENCES epics(id) ON DELETE SET NULL,
            FOREIGN KEY (assignee_id) REFERENCES assignees(id) ON DELETE SET NULL
        )",
        [],
    )?;
    
    // Dependencies table
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
    
    // External references table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS external_refs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL,
            ref_type TEXT NOT NULL,
            reference TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        )",
        [],
    )?;
    
    // Create indexes for performance
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
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_external_refs_task ON external_refs(task_id)",
        [],
    )?;
    
    Ok(())
}
