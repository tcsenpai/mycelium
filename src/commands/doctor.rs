use colored::Colorize;
use crate::commands::{ensure_initialized, SUCCESS_PREFIX, ERROR_PREFIX, INFO_PREFIX, WARNING_PREFIX};
use crate::db::Database;
use crate::error::Result;
use std::fs;

pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    pub fixable: bool,
}

pub enum CheckStatus {
    Ok,
    Warning,
    Error,
}

impl CheckResult {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Ok,
            message: message.into(),
            fixable: false,
        }
    }

    fn warning(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warning,
            message: message.into(),
            fixable: false,
        }
    }

    fn error(name: impl Into<String>, message: impl Into<String>, fixable: bool) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Error,
            message: message.into(),
            fixable,
        }
    }
}

pub fn execute(fix: bool, quiet: bool) -> Result<()> {
    if !quiet {
        println!("{} Running mycelium health checks...", INFO_PREFIX.blue());
        println!();
    }

    let mut results = Vec::new();
    let mut fixed_count = 0;
    let mut fixable_count = 0;

    // Run all checks
    results.push(check_project_initialized()?);
    results.push(check_database_accessible()?);
    results.push(check_database_integrity()?);
    results.push(check_orphaned_tasks()?);
    results.push(check_circular_dependencies()?);
    results.push(check_gitignore()?);
    results.push(check_wal_files()?);
    results.push(check_schema_version()?);

    // Count fixable issues
    for result in &results {
        if matches!(result.status, CheckStatus::Error) && result.fixable {
            fixable_count += 1;
        }
    }

    // Try to fix issues if requested
    if fix && fixable_count > 0 {
        if !quiet {
            println!("{} Attempting to fix {} issue(s)...", INFO_PREFIX.blue(), fixable_count);
            println!();
        }

        for result in &results {
            if matches!(result.status, CheckStatus::Error) && result.fixable {
                match try_fix(&result.name) {
                    Ok(true) => {
                        fixed_count += 1;
                        if !quiet {
                            println!("{} Fixed: {}", SUCCESS_PREFIX.green(), result.name);
                        }
                    }
                    Ok(false) => {
                        if !quiet {
                            println!("{} Could not fix: {}", WARNING_PREFIX.yellow(), result.name);
                        }
                    }
                    Err(e) => {
                        if !quiet {
                            println!("{} Error fixing {}: {}", ERROR_PREFIX.red(), result.name, e);
                        }
                    }
                }
            }
        }

        if !quiet {
            println!();
        }
    }

    // Display results
    if !quiet {
        display_results(&results);
    }

    // Summary
    let ok_count = results.iter().filter(|r| matches!(r.status, CheckStatus::Ok)).count();
    let warning_count = results.iter().filter(|r| matches!(r.status, CheckStatus::Warning)).count();
    let error_count = results.iter().filter(|r| matches!(r.status, CheckStatus::Error)).count();

    if !quiet {
        println!();
        println!("Summary: {} OK, {} warnings, {} errors", 
            ok_count.to_string().green(),
            warning_count.to_string().yellow(),
            error_count.to_string().red()
        );

        if fixable_count > 0 && !fix {
            println!();
            println!("{} {} issue(s) can be fixed automatically. Run with --fix to apply.", 
                INFO_PREFIX.blue(), fixable_count);
        }

        if fix && fixed_count > 0 {
            println!();
            println!("{} Fixed {} issue(s)", SUCCESS_PREFIX.green(), fixed_count);
        }
    }

    // Exit with error code if there are unfixable errors
    if error_count > fixed_count {
        std::process::exit(1);
    }

    Ok(())
}

fn display_results(results: &[CheckResult]) {
    for result in results {
        let (icon, color) = match result.status {
            CheckStatus::Ok => ("✓", "green"),
            CheckStatus::Warning => ("⚠", "yellow"),
            CheckStatus::Error => ("✗", "red"),
        };

        let fix_indicator = if result.fixable { " [fixable]" } else { "" };
        
        println!("{} {}: {}{}", 
            icon.color(color),
            result.name.bold(),
            result.message,
            fix_indicator.dimmed()
        );
    }
}

fn check_project_initialized() -> Result<CheckResult> {
    let mycelium_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium");

    if !mycelium_dir.exists() {
        return Ok(CheckResult::error(
            "Project initialized",
            "No .mycelium/ directory found. Run 'myc init' first.",
            false
        ));
    }

    Ok(CheckResult::ok("Project initialized", ".mycelium/ directory exists"))
}

fn check_database_accessible() -> Result<CheckResult> {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("mycelium.db");

    if !db_path.exists() {
        return Ok(CheckResult::error(
            "Database accessible",
            "Database file not found",
            true
        ));
    }

    match Database::open(&db_path) {
        Ok(_) => Ok(CheckResult::ok("Database accessible", "Can open database")),
        Err(e) => Ok(CheckResult::error(
            "Database accessible",
            format!("Cannot open database: {}", e),
            false
        )),
    }
}

fn check_database_integrity() -> Result<CheckResult> {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("mycelium.db");

    if !db_path.exists() {
        return Ok(CheckResult::warning("Database integrity", "Database doesn't exist, skipping check"));
    }

    match Database::open(&db_path) {
        Ok(db) => {
            // Try to query each table to verify integrity
            let conn = db.get_conn();
            
            let tables = ["epics", "tasks", "assignees", "dependencies", "external_refs"];
            for table in &tables {
                // Check if table exists by querying sqlite_master
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
                    [table],
                    |row| row.get(0),
                ).unwrap_or(false);
                
                if !exists {
                    return Ok(CheckResult::error(
                        "Database integrity",
                        format!("Table '{}' does not exist", table),
                        false
                    ));
                }
                
                // Try to get column info to verify structure
                let result: std::result::Result<String, rusqlite::Error> = conn.query_row(
                    &format!("SELECT sql FROM sqlite_master WHERE type='table' AND name='{}'", table),
                    [],
                    |row| row.get(0),
                );
                
                if let Err(e) = result {
                    return Ok(CheckResult::error(
                        "Database integrity",
                        format!("Table '{}' structure check failed: {}", table, e),
                        false
                    ));
                }
            }
            
            Ok(CheckResult::ok("Database integrity", "All tables accessible"))
        }
        Err(e) => Ok(CheckResult::error(
            "Database integrity",
            format!("Cannot verify: {}", e),
            false
        )),
    }
}

fn check_orphaned_tasks() -> Result<CheckResult> {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("mycelium.db");

    if !db_path.exists() {
        return Ok(CheckResult::warning("Orphaned tasks", "Database doesn't exist, skipping check"));
    }

    match Database::open(&db_path) {
        Ok(db) => {
            let conn = db.get_conn();
            
            // Check for tasks with non-existent epic_id
            let orphaned: i64 = conn.query_row(
                "SELECT COUNT(*) FROM tasks t 
                 LEFT JOIN epics e ON t.epic_id = e.id 
                 WHERE t.epic_id IS NOT NULL AND e.id IS NULL",
                [],
                |row| row.get(0),
            ).unwrap_or(0);

            if orphaned > 0 {
                Ok(CheckResult::warning(
                    "Orphaned tasks",
                    format!("{} task(s) reference non-existent epics", orphaned),
                ))
            } else {
                Ok(CheckResult::ok("Orphaned tasks", "No orphaned tasks found"))
            }
        }
        Err(_) => Ok(CheckResult::warning("Orphaned tasks", "Cannot check")),
    }
}

fn check_circular_dependencies() -> Result<CheckResult> {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("mycelium.db");

    if !db_path.exists() {
        return Ok(CheckResult::warning("Circular dependencies", "Database doesn't exist, skipping check"));
    }

    match Database::open(&db_path) {
        Ok(db) => {
            let tasks = db.list_tasks(None, None, None, None, false, false, None)?;
            
            for task in &tasks {
                if let Ok(chain) = db.get_all_dependencies(task.id) {
                    if chain.all_dependencies.contains(&task.id) {
                        return Ok(CheckResult::error(
                            "Circular dependencies",
                            format!("Task #{} has circular dependency", task.id),
                            true
                        ));
                    }
                }
            }
            
            Ok(CheckResult::ok("Circular dependencies", "No circular dependencies found"))
        }
        Err(_) => Ok(CheckResult::warning("Circular dependencies", "Cannot check")),
    }
}

fn check_gitignore() -> Result<CheckResult> {
    let gitignore_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join(".gitignore");

    if !gitignore_path.exists() {
        return Ok(CheckResult::error(
            "Gitignore",
            ".mycelium/.gitignore not found",
            true
        ));
    }

    match fs::read_to_string(&gitignore_path) {
        Ok(content) => {
            if content.contains("*.db-wal") && content.contains("*.db-shm") {
                Ok(CheckResult::ok("Gitignore", "WAL files are ignored"))
            } else {
                Ok(CheckResult::error(
                    "Gitignore",
                    ".gitignore missing WAL file entries",
                    true
                ))
            }
        }
        Err(_) => Ok(CheckResult::error("Gitignore", "Cannot read .gitignore", false)),
    }
}

fn check_wal_files() -> Result<CheckResult> {
    let mycelium_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium");

    let wal_exists = mycelium_dir.join("mycelium.db-wal").exists();
    let shm_exists = mycelium_dir.join("mycelium.db-shm").exists();

    if wal_exists || shm_exists {
        Ok(CheckResult::warning(
            "WAL files",
            "WAL files present (normal during operation, can be checkpointed)"
        ))
    } else {
        Ok(CheckResult::ok("WAL files", "No WAL files (clean)"))
    }
}

fn check_schema_version() -> Result<CheckResult> {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("mycelium.db");

    if !db_path.exists() {
        return Ok(CheckResult::warning("Schema version", "Database doesn't exist"));
    }

    match Database::open(&db_path) {
        Ok(db) => {
            let conn = db.get_conn();
            
            // Check if we can query the tags column (schema v2)
            match conn.execute("SELECT tags FROM tasks LIMIT 1", []) {
                Ok(_) => Ok(CheckResult::ok("Schema version", "Database schema is up to date (v2)")),
                Err(_) => Ok(CheckResult::error(
                    "Schema version",
                    "Database schema is outdated. Missing 'tags' column.",
                    true
                )),
            }
        }
        Err(e) => Ok(CheckResult::error(
            "Schema version",
            format!("Cannot check: {}", e),
            false
        )),
    }
}

fn try_fix(check_name: &str) -> Result<bool> {
    match check_name {
        "Database accessible" => fix_database(),
        "Gitignore" => fix_gitignore(),
        "Schema version" => fix_schema(),
        "Circular dependencies" => fix_circular_deps(),
        _ => Ok(false),
    }
}

fn fix_database() -> Result<bool> {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("mycelium.db");

    // Try to recreate the database
    Database::open(&db_path)?;
    Ok(true)
}

fn fix_gitignore() -> Result<bool> {
    let gitignore_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join(".gitignore");

    let content = r#"# Mycelium database
# The database file is git-trackable but WAL files are not
*.db-wal
*.db-shm
# Temporary files
*.tmp
"#;

    fs::write(gitignore_path, content)?;
    Ok(true)
}

fn fix_schema() -> Result<bool> {
    let db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("mycelium.db");

    if !db_path.exists() {
        return Ok(false);
    }

    let mut db = Database::open(&db_path)?;
    
    // Re-run migrations
    db.migrate()?;
    
    Ok(true)
}

fn fix_circular_deps() -> Result<bool> {
    // For circular deps, we'd need to break the cycle
    // This is complex and might need user input
    // For now, just report that it needs manual intervention
    Ok(false)
}
