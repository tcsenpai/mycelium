use rusqlite::Connection;
use crate::error::Result;

const CURRENT_VERSION: i32 = 6;

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    create_migrations_table(conn)?;
    
    let version = get_current_version(conn)?;
    
    if version < 1 {
        migrate_v1(conn)?;
        set_version(conn, 1)?;
    }
    
    if version < 2 {
        migrate_v2(conn)?;
        set_version(conn, 2)?;
    }
    
    if version < 3 {
        migrate_v3(conn)?;
        set_version(conn, 3)?;
    }

    if version < 4 {
        migrate_v4(conn)?;
        set_version(conn, 4)?;
    }

    if version < 5 {
        migrate_v5(conn)?;
        set_version(conn, 5)?;
    }

    if version < 6 {
        migrate_v6(conn)?;
        set_version(conn, 6)?;
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

fn migrate_v2(conn: &Connection) -> Result<()> {
    // Add tags column to tasks
    conn.execute(
        "ALTER TABLE tasks ADD COLUMN tags TEXT",
        [],
    )?;
    
    // Create index for tag search
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tasks_tags ON tasks(tags)",
        [],
    )?;
    
    Ok(())
}

fn migrate_v3(conn: &Connection) -> Result<()> {
    // Create task_notes table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS task_notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id INTEGER NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
        )",
        [],
    )?;
    
    // Create index for task notes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_task_notes_task ON task_notes(task_id)",
        [],
    )?;

    Ok(())
}

fn migrate_v4(conn: &Connection) -> Result<()> {
    // Linear sync mapping table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS linear_sync (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            local_task_id INTEGER NOT NULL,
            linear_issue_id TEXT NOT NULL,
            linear_issue_identifier TEXT,
            last_synced_at TEXT NOT NULL,
            last_local_hash TEXT NOT NULL,
            last_remote_hash TEXT NOT NULL,
            sync_direction TEXT NOT NULL DEFAULT 'both',
            FOREIGN KEY (local_task_id) REFERENCES tasks(id) ON DELETE CASCADE,
            UNIQUE(local_task_id),
            UNIQUE(linear_issue_id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_linear_sync_issue ON linear_sync(linear_issue_id)",
        [],
    )?;

    Ok(())
}

fn migrate_v5(conn: &Connection) -> Result<()> {
    // Add notes, user_info, and agent_questions columns to tasks
    conn.execute("ALTER TABLE tasks ADD COLUMN notes TEXT", [])?;
    conn.execute("ALTER TABLE tasks ADD COLUMN user_info TEXT", [])?;
    conn.execute("ALTER TABLE tasks ADD COLUMN agent_questions TEXT", [])?;

    // Add notes, user_info, and agent_questions columns to epics
    conn.execute("ALTER TABLE epics ADD COLUMN notes TEXT", [])?;
    conn.execute("ALTER TABLE epics ADD COLUMN user_info TEXT", [])?;
    conn.execute("ALTER TABLE epics ADD COLUMN agent_questions TEXT", [])?;

    // Create epic_notes table (parallel to task_notes)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS epic_notes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            epic_id INTEGER NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (epic_id) REFERENCES epics(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_epic_notes_epic ON epic_notes(epic_id)",
        [],
    )?;

    Ok(())
}

fn migrate_v6(conn: &Connection) -> Result<()> {
    // Add is_knowledge and key_questions to tasks
    conn.execute("ALTER TABLE tasks ADD COLUMN is_knowledge INTEGER NOT NULL DEFAULT 0", [])?;
    conn.execute("ALTER TABLE tasks ADD COLUMN key_questions TEXT", [])?;

    // Add is_knowledge and key_questions to epics
    conn.execute("ALTER TABLE epics ADD COLUMN is_knowledge INTEGER NOT NULL DEFAULT 0", [])?;
    conn.execute("ALTER TABLE epics ADD COLUMN key_questions TEXT", [])?;

    // Embeddings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS embeddings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            entity_type TEXT NOT NULL,
            entity_id INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            embedding BLOB NOT NULL,
            model_version TEXT,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_embeddings_entity ON embeddings(entity_type, entity_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_embeddings_hash ON embeddings(content_hash)",
        [],
    )?;

    Ok(())
}
