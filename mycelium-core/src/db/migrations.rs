use crate::error::Result;
use rusqlite::Connection;

// Latest schema version. Referenced by tests; the migration gate uses literal
// version numbers, so this is documentation + a test anchor.
#[allow(dead_code)]
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

    // Cross-branch safety net. The linear `version < N` gate above skips a
    // migration whose number was already recorded on a *different* branch
    // (e.g. branch A's v6 = embeddings, branch B's v6 = followups: switching
    // A→B leaves _migrations at 6 so B's v6 never runs, and its table is
    // missing). Re-asserting every table/index/column idempotently repairs that
    // mismatch without a migration-ID rewrite.
    //
    // Guard on a sentinel table so the healthy path issues ZERO writes on
    // startup — re-running DDL on every open added write contention under
    // concurrent CLI invocations. We only re-assert when something is actually
    // missing (the branch-switch repair case).
    // ponytail: idempotent DDL re-assert, upgrade to checksum-tracked
    // migrations if per-branch column drift ever appears.
    if !table_exists(conn, "followups")? {
        ensure_schema(conn)?;
    }

    Ok(())
}

/// Whether a table exists (used to gate the schema re-assert).
fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [name],
        |row| row.get(0),
    )?;
    Ok(exists)
}

/// Column names present on a table, via PRAGMA table_info.
fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<String>, _>>()?;
    Ok(cols)
}

/// Add a column only if it does not already exist. ALTER TABLE ADD COLUMN is
/// not idempotent (errors on duplicate), so guard it — required for the
/// cross-branch re-assert to be safe.
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, ddl: &str) -> Result<()> {
    if !table_columns(conn, table)?.iter().any(|c| c == column) {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {ddl}"), [])?;
    }
    Ok(())
}

/// Idempotently re-assert the full current schema. Every CREATE uses
/// IF NOT EXISTS; every column add is guarded. Safe to run on every startup.
fn ensure_schema(conn: &Connection) -> Result<()> {
    // Tables + indexes (all IF NOT EXISTS) — re-run the creating migrations.
    migrate_v1(conn)?;
    migrate_v3(conn)?;
    migrate_v4(conn)?;
    migrate_v6(conn)?;
    // epic_notes table from v5 (also IF NOT EXISTS).
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

    // Added columns (ALTER — guarded, since ALTER ADD COLUMN is not idempotent).
    add_column_if_missing(conn, "tasks", "tags", "tags TEXT")?; // v2
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tasks_tags ON tasks(tags)",
        [],
    )?;
    for (col, ddl) in [
        ("notes", "notes TEXT"),
        ("user_info", "user_info TEXT"),
        ("agent_questions", "agent_questions TEXT"),
    ] {
        add_column_if_missing(conn, "tasks", col, ddl)?; // v5
        add_column_if_missing(conn, "epics", col, ddl)?; // v5
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

    let result: std::result::Result<Option<i32>, rusqlite::Error> =
        conn.query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0));

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
    conn.execute("ALTER TABLE tasks ADD COLUMN tags TEXT", [])?;

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
    // Follow-ups: lightweight scratch table for non-blocking
    // "oh-by-the-way" items captured mid-work. Independent of tasks/epics.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS followups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            body TEXT NOT NULL,
            title TEXT,
            status TEXT NOT NULL DEFAULT 'open',
            closure_reason TEXT,
            created_at TEXT NOT NULL,
            closed_at TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_followups_status ON followups(status)",
        [],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [name],
            |row| row.get::<_, bool>(0),
        )
        .unwrap()
    }

    #[test]
    fn fresh_db_gets_full_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        for t in [
            "epics",
            "tasks",
            "dependencies",
            "task_notes",
            "epic_notes",
            "followups",
        ] {
            assert!(table_exists(&conn, t), "missing table {t}");
        }
        assert_eq!(get_current_version(&conn).unwrap(), CURRENT_VERSION);
    }

    #[test]
    fn cross_branch_collision_self_heals() {
        // Simulate: another branch recorded version 6 with a DIFFERENT v6
        // (so followups was never created here). The linear gate would skip
        // v6; ensure_schema must still create the missing table.
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        conn.execute("DROP TABLE followups", []).unwrap();
        assert!(!table_exists(&conn, "followups"));
        // _migrations still says version 6.
        assert_eq!(get_current_version(&conn).unwrap(), 6);

        // Re-run: gate skips (version already 6), ensure_schema repairs.
        run_migrations(&mut conn).unwrap();
        assert!(table_exists(&conn, "followups"), "followups not repaired");
    }

    #[test]
    fn ensure_schema_is_idempotent() {
        // Running twice must not error (guards ALTER ADD COLUMN duplication).
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap();
        assert!(table_columns(&conn, "tasks")
            .unwrap()
            .iter()
            .any(|c| c == "tags"));
    }
}
