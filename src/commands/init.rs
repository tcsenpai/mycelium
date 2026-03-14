use std::fs;
use std::path::Path;
use colored::Colorize;
use crate::commands::{SUCCESS_PREFIX, INFO_PREFIX};
use crate::db::Database;
use crate::error::Result;

const AGENTS_MD_CONTENT: &str = r#"
## Project Management with Mycelium

This project uses [Mycelium](https://github.com/tcsenpai/mycelium) (`myc`) for task and epic management.

### Quick Reference

```bash
# Initialize mycelium in this project (creates .mycelium/ directory)
myc init

# Create an epic (a large body of work)
myc epic create --title "Feature X" --description "Build feature X"

# Create tasks within an epic
myc task create --title "Implement Y" --description "Build the implementation for Y" --epic 1 --priority high --due 2025-12-31

# Task priorities: low, medium, high, critical
# Task status: open, closed

# List tasks
myc task list
myc task list --epic 1
myc task list --overdue
myc task list --blocked

# Manage dependencies (task 1 blocks task 2)
myc task link blocks --task 1 2
myc deps show 2

# Close tasks (blocked tasks cannot be closed without --force)
myc task close 1

# Assign tasks
myc assignee create --name "Alice" --github "alice"
myc task assign 1 1

# Link to external resources
myc task link github-issue --task 1 "owner/repo#123"
myc task link github-pr --task 1 "owner/repo#456"
myc task link url --task 1 "https://example.com"

# Project overview
myc summary

# Export data
myc export json
myc export csv
```

### Data Model

- **Epic**: A large body of work with a title and optional description (e.g., a feature or milestone)
- **Task**: A unit of work with a title and optional description, optionally linked to an epic
- **Dependency**: Task A blocks Task B (B cannot close until A is closed)
- **Assignee**: Person assigned to a task (can have GitHub username)
- **External Ref**: Link to GitHub issues/PRs or URLs

### Git Tracking

The `.mycelium/` directory contains the SQLite database and should be committed to git:

```bash
git add .mycelium/
git commit -m "Add mycelium project tracking"
```

### For AI Agents

When working on this project:

1. Check existing tasks: `myc task list`
2. Check blocked tasks: `myc task list --blocked`
3. Create tasks for new work: `myc task create --title "..." --description "..." --epic N`
4. Mark tasks complete when done: `myc task close N`
5. Use `--format json` for machine-readable output: `myc task list --format json`
"#;

pub fn execute() -> Result<()> {
    let mycelium_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium");
    
    let db_path = mycelium_dir.join("mycelium.db");
    
    if mycelium_dir.exists() {
        println!("{} Mycelium project already initialized", INFO_PREFIX.blue());
        return Ok(());
    }
    
    // Create directory
    fs::create_dir_all(&mycelium_dir)?;
    
    // Create .gitignore
    let gitignore_content = r#"# Mycelium database
# The database file is git-trackable but WAL files are not
*.db-wal
*.db-shm
# Temporary files
*.tmp
"#;
    fs::write(mycelium_dir.join(".gitignore"), gitignore_content)?;
    
    // Initialize database
    Database::open(&db_path)?;
    
    // Create or append to AGENTS.md
    let agents_md_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("AGENTS.md");
    
    if agents_md_path.exists() {
        // Append to existing file
        let existing = fs::read_to_string(&agents_md_path)?;
        if !existing.contains("Mycelium") {
            fs::write(&agents_md_path, format!("{}\n{}", existing, AGENTS_MD_CONTENT))?;
            println!("{} Updated AGENTS.md with mycelium instructions", INFO_PREFIX.blue());
        }
    } else {
        // Create new file
        fs::write(&agents_md_path, format!("# Agent Instructions\n{}", AGENTS_MD_CONTENT))?;
        println!("{} Created AGENTS.md with mycelium instructions", INFO_PREFIX.blue());
    }
    
    println!("{} Mycelium project initialized", SUCCESS_PREFIX.green());
    println!("  Database: {}", db_path.display());
    println!("  Git tracking: Add {} to your repo", ".mycelium/".cyan());
    
    Ok(())
}
