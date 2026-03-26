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

## Mental Frameworks for Mycelium Usage

### 1. INVEST — Task Quality Gate

Before creating or updating any task, validate it against these criteria.
A task that fails more than one is not ready to be written.

| Criterion | Rule |
|---|---|
| **Independent** | Can be completed without unblocking other tasks first |
| **Negotiable** | The *what* is fixed; the *how* remains open |
| **Valuable** | Produces a verifiable, concrete outcome |
| **Estimable** | If you cannot size it, it is too vague or too large |
| **Small** | If it spans more than one work cycle, split it |
| **Testable** | Has an explicit, binary done condition |

> If a task fails **Estimable** or **Testable**, convert it to an Epic and decompose.

---

### 2. DAG — Dependency Graph Thinking

Before scheduling or prioritizing, model the implicit dependency graph.

**Rules:**
- No task moves to `in_progress` if it has an unresolved upstream blocker
- Priority is a function of both urgency **and fan-out** (how many tasks does completing this one unlock?)
- Always work the **critical path** first — not the task that feels most urgent

**Prioritization heuristic:**
```
score = urgency + (blocked_tasks_count × 1.5)
```

When creating a task, explicitly ask: *"What does this block, and what blocks this?"*
Set dependency links in Mycelium before touching status.

---

### 3. Principle of Minimal Surprise (PMS)

Mycelium's state must remain predictable and auditable at all times.

**Rules:**
- **Prefer idempotent operations** — update before you create; never duplicate
- **Check before write** — search for an equivalent item before creating a new one
- **Always annotate mutations** — every status change, priority shift, or reassignment must carry an explicit `reason` field
- **No orphan tasks** — every task must be linked to an Epic; every Epic to a strategic goal
- Deletions are a last resort; prefer `cancelled` status with a reason

> The state of Mycelium after any operation must be explainable to another agent with zero context.
"#;

pub fn execute(force_agents: bool) -> Result<()> {
    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mycelium_dir = cwd.join(".mycelium");
    let db_path = mycelium_dir.join("mycelium.db");
    let agents_md_path = cwd.join("AGENTS.md");

    if mycelium_dir.exists() && !force_agents {
        println!("{} Mycelium project already initialized", INFO_PREFIX.blue());
        return Ok(());
    }

    if !mycelium_dir.exists() {
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

        println!("{} Mycelium project initialized", SUCCESS_PREFIX.green());
        println!("  Database: {}", db_path.display());
        println!("  Git tracking: Add {} to your repo", ".mycelium/".cyan());
    }

    // Create, append, or force-regenerate AGENTS.md
    if force_agents {
        if agents_md_path.exists() {
            // Remove existing Mycelium section and rewrite it
            let existing = fs::read_to_string(&agents_md_path)?;
            let cleaned = remove_mycelium_section(&existing);
            if cleaned.trim().is_empty() {
                fs::write(&agents_md_path, format!("# Agent Instructions\n{}", AGENTS_MD_CONTENT))?;
            } else {
                fs::write(&agents_md_path, format!("{}\n{}", cleaned.trim_end(), AGENTS_MD_CONTENT))?;
            }
        } else {
            fs::write(&agents_md_path, format!("# Agent Instructions\n{}", AGENTS_MD_CONTENT))?;
        }
        println!("{} Regenerated AGENTS.md with mycelium instructions", SUCCESS_PREFIX.green());
    } else if agents_md_path.exists() {
        let existing = fs::read_to_string(&agents_md_path)?;
        if !existing.contains("Mycelium") {
            fs::write(&agents_md_path, format!("{}\n{}", existing, AGENTS_MD_CONTENT))?;
            println!("{} Updated AGENTS.md with mycelium instructions", INFO_PREFIX.blue());
        }
    } else {
        fs::write(&agents_md_path, format!("# Agent Instructions\n{}", AGENTS_MD_CONTENT))?;
        println!("{} Created AGENTS.md with mycelium instructions", INFO_PREFIX.blue());
    }

    Ok(())
}

/// Remove the Mycelium section from AGENTS.md content so it can be regenerated.
fn remove_mycelium_section(content: &str) -> String {
    let mut result = String::new();
    let mut in_mycelium_section = false;

    for line in content.lines() {
        if line.contains("## Project Management with Mycelium") || line.contains("## Mental Frameworks for Mycelium Usage") {
            in_mycelium_section = true;
            continue;
        }
        // A new H2 that isn't part of Mycelium ends the skip
        if in_mycelium_section && line.starts_with("## ") && !line.contains("Mental Frameworks for Mycelium") {
            in_mycelium_section = false;
        }
        if !in_mycelium_section {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}
