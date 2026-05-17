use crate::commands::{INFO_PREFIX, SUCCESS_PREFIX};
use crate::db::Database;
use crate::error::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Bump this whenever AGENTS_MD_CONTENT changes. `myc prime-agents`
/// without --force only updates when the embedded marker version differs.
const AGENTS_MD_VERSION: u32 = 3;
const AGENTS_MARKER_START: &str = "<!-- myc:agents-start";
const AGENTS_MARKER_END: &str = "<!-- myc:agents-end -->";

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

### Follow-ups (`myc followup`, alias `myc fu`)

Lightweight scratch table for non-blocking "oh-by-the-way" items
captured mid-work — bugs, questions, ideas, things the user should look
at later. **Separate from tasks** (no epic/priority/deps/assignee). Most
follow-ups are resolved by the user, not the agent.

```bash
myc followup add "body text"                # capture (body required)
myc followup add "body text" --title "tag"  # optional short title
myc fu add "short form alias works too"

myc followup list                           # all (default)
myc followup list -o                        # only active (open + in_progress)
myc followup list -c                        # only closed (done + wontfix)
myc followup list --status done             # exact status

myc followup show <id>                      # full detail
myc followup next                           # lowest-ID active (agent loop)
myc followup count                          # JSON: {open, in_progress, done, wontfix}

myc followup start <id>                     # → in_progress
myc followup done <id> [--reason "..."]     # → done
myc followup wontfix <id> [--reason "..."]  # → wontfix
myc followup reopen <id>                    # → open

myc followup edit <id> --body "new body" [--title -|"new title"]
myc followup append <id> "more context"     # timestamped, preserves existing
myc followup rm <id> [--force]
myc followup promote <id> [--epic N] [--priority high]  # convert to task
```

**Agent rule — end-of-task follow-up check** (MANDATORY)

At the end of every mycelium-tracked unit of work (closing a task,
finishing a user-requested change that touched myc state), the agent
MUST:

1. Run `myc followup list --format json` (or `myc followup count
   --format json`).
2. If `active > 0`, surface them to the user before wrapping:
   > "Before we wrap — N open follow-up(s): [titles/bodies]. Want me to
   > handle any now, or leave for later?"
3. **Never silently process them.** Always ask.

`myc task close` itself also prints a one-line reminder, but the agent
should still proactively check.

Use `myc followup add` during work to capture anything you notice but
shouldn't act on right now.

### For AI Agents

When working on this project:

1. Check existing tasks: `myc task list`
2. Check blocked tasks: `myc task list --blocked`
3. Create tasks for new work: `myc task create --title "..." --description "..." --epic N`
4. Capture incidental observations as follow-ups: `myc followup add "..."`
5. At end of task: `myc followup list` and surface open ones to the user
6. Mark tasks complete when done: `myc task close N`
7. Use `--format json` for machine-readable output: `myc task list --format json`

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

/// Ensure `.mycelium/` exists with gitignore and an initialized DB.
/// Returns true if the project was just created, false if it already existed.
fn ensure_project_initialized(mycelium_dir: &Path) -> Result<bool> {
    if mycelium_dir.exists() {
        return Ok(false);
    }

    fs::create_dir_all(mycelium_dir)?;

    let gitignore_content = r#"# Mycelium database
# The database file is git-trackable but WAL files are not
*.db-wal
*.db-shm
# Temporary files
*.tmp
"#;
    fs::write(mycelium_dir.join(".gitignore"), gitignore_content)?;

    let db_path = mycelium_dir.join("mycelium.db");
    Database::open(&db_path)?;

    println!("{} Mycelium project initialized", SUCCESS_PREFIX.green());
    println!("  Database: {}", db_path.display());
    println!("  Git tracking: Add {} to your repo", ".mycelium/".cyan());

    Ok(true)
}

pub fn execute(force_init: bool) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mycelium_dir = cwd.join(".mycelium");
    let agents_md_path = cwd.join("AGENTS.md");

    if mycelium_dir.exists() && !force_init {
        println!(
            "{} Mycelium project already initialized",
            INFO_PREFIX.blue()
        );
        return Ok(());
    }

    ensure_project_initialized(&mycelium_dir)?;

    // Create AGENTS.md if it doesn't exist
    if !agents_md_path.exists() {
        fs::write(
            &agents_md_path,
            format!("# Agent Instructions\n{}", marker_block()),
        )?;
        println!(
            "{} Created AGENTS.md with mycelium instructions",
            INFO_PREFIX.blue()
        );
    } else {
        let existing = fs::read_to_string(&agents_md_path)?;
        match apply_marker_block(&existing, false) {
            Some((updated, action)) => {
                fs::write(&agents_md_path, updated)?;
                println!("{} {} AGENTS.md mycelium block", INFO_PREFIX.blue(), action);
            }
            None => {
                println!(
                    "{} AGENTS.md mycelium block already at v{} — no change",
                    INFO_PREFIX.blue(),
                    AGENTS_MD_VERSION
                );
            }
        }
    }

    Ok(())
}

pub fn execute_prime_agents(force: bool, path: Option<&Path>) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mycelium_dir = cwd.join(".mycelium");
    let agents_md_path = path
        .map(|p| cwd.join(p))
        .unwrap_or_else(|| cwd.join("AGENTS.md"));

    ensure_project_initialized(&mycelium_dir)?;

    if !agents_md_path.exists() {
        fs::write(
            &agents_md_path,
            format!("# Agent Instructions\n{}", marker_block()),
        )?;
        println!(
            "{} Created AGENTS.md with mycelium instructions (v{})",
            SUCCESS_PREFIX.green(),
            AGENTS_MD_VERSION
        );
        return Ok(());
    }

    let existing = fs::read_to_string(&agents_md_path)?;
    match apply_marker_block(&existing, force) {
        Some((updated, action)) => {
            fs::write(&agents_md_path, updated)?;
            println!(
                "{} {} AGENTS.md mycelium block (v{})",
                SUCCESS_PREFIX.green(),
                action,
                AGENTS_MD_VERSION
            );
        }
        None => {
            println!(
                "{} AGENTS.md mycelium block already at v{} — no change (use --force to regenerate)",
                INFO_PREFIX.blue(), AGENTS_MD_VERSION
            );
        }
    }

    Ok(())
}

/// Build the wrapped marker block with embedded version.
fn marker_block() -> String {
    format!(
        "\n{} v={} -->\n{}\n{}\n",
        AGENTS_MARKER_START,
        AGENTS_MD_VERSION,
        AGENTS_MD_CONTENT.trim(),
        AGENTS_MARKER_END,
    )
}

/// Locate `(start_line_index, end_line_index_inclusive, embedded_version)` for
/// the mycelium marker block in `content`. None if no markers.
fn find_marker_block(content: &str) -> Option<(usize, usize, Option<u32>)> {
    let mut start = None;
    let mut end = None;
    let mut version = None;
    for (idx, line) in content.lines().enumerate() {
        if line.contains(AGENTS_MARKER_START) {
            start = Some(idx);
            // parse v=N
            if let Some(v_pos) = line.find("v=") {
                let rest = &line[v_pos + 2..];
                let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(n) = num.parse::<u32>() {
                    version = Some(n);
                }
            }
        }
        if line.contains(AGENTS_MARKER_END) && start.is_some() {
            end = Some(idx);
            break;
        }
    }
    match (start, end) {
        (Some(s), Some(e)) if e >= s => Some((s, e, version)),
        _ => None,
    }
}

/// Returns (new_content, action) when a write is needed, or None when no change.
/// `force=true` always replaces the block (and migrates legacy unmarked content).
fn apply_marker_block(existing: &str, force: bool) -> Option<(String, &'static str)> {
    if let Some((s, e, ver)) = find_marker_block(existing) {
        // Markers present
        if !force && ver == Some(AGENTS_MD_VERSION) {
            return None;
        }
        let lines: Vec<&str> = existing.lines().collect();
        let before = lines[..s].join("\n");
        let after = if e + 1 < lines.len() {
            lines[e + 1..].join("\n")
        } else {
            String::new()
        };
        let mut out = String::new();
        if !before.is_empty() {
            out.push_str(&before);
            out.push('\n');
        }
        out.push_str(marker_block().trim_start_matches('\n'));
        if !after.is_empty() {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&after);
        }
        if !out.ends_with('\n') {
            out.push('\n');
        }
        let action = if ver.is_none() {
            "Wrapped"
        } else if ver == Some(AGENTS_MD_VERSION) {
            "Regenerated"
        } else {
            "Upgraded"
        };
        Some((out, action))
    } else {
        // Legacy file (no markers). Migrate: strip old heuristic-detected
        // sections, then append marker block.
        let cleaned = remove_mycelium_section_legacy(existing);
        let trimmed = cleaned.trim_end();
        let new_content = if trimmed.is_empty() {
            format!("# Agent Instructions\n{}", marker_block())
        } else {
            format!("{}\n{}", trimmed, marker_block())
        };
        if new_content == existing {
            None
        } else {
            Some((new_content, "Migrated to marker-block"))
        }
    }
}

/// Legacy heuristic: strip `## Project Management with Mycelium` and
/// `## Mental Frameworks for Mycelium Usage` sections. Only used during
/// one-time migration from pre-marker AGENTS.md files.
fn remove_mycelium_section_legacy(content: &str) -> String {
    let mut result = String::new();
    let mut in_mycelium_section = false;

    for line in content.lines() {
        if line.contains("## Project Management with Mycelium")
            || line.contains("## Mental Frameworks for Mycelium Usage")
        {
            in_mycelium_section = true;
            continue;
        }
        if in_mycelium_section
            && line.starts_with("## ")
            && !line.contains("Mental Frameworks for Mycelium")
        {
            in_mycelium_section = false;
        }
        if !in_mycelium_section {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_wraps_legacy_unmarked_file() {
        let original = "# Agent Instructions\n\n## Other Project Notes\n\nKeep these.\n\n## Project Management with Mycelium\n\nOld content.\n\n## Mental Frameworks for Mycelium Usage\n\nOld frameworks.\n";
        let (updated, action) = apply_marker_block(original, false).expect("should update");
        assert_eq!(action, "Migrated to marker-block");
        assert!(updated.contains("## Other Project Notes"));
        assert!(updated.contains("Keep these"));
        assert!(updated.contains(AGENTS_MARKER_START));
        assert!(updated.contains(AGENTS_MARKER_END));
        // Old content gone
        assert!(!updated.contains("Old content"));
        assert!(!updated.contains("Old frameworks"));
    }

    #[test]
    fn no_change_when_marker_at_current_version() {
        let original = format!(
            "# Agent Instructions\n\n## Other\n\nfoo\n\n{} v={} -->\nhello\n{}\n",
            AGENTS_MARKER_START, AGENTS_MD_VERSION, AGENTS_MARKER_END
        );
        assert!(apply_marker_block(&original, false).is_none());
    }

    #[test]
    fn upgrade_when_version_differs() {
        let original = format!(
            "# Agent Instructions\n\n## Other\n\nfoo\n\n{} v=1 -->\nold block\n{}\n",
            AGENTS_MARKER_START, AGENTS_MARKER_END
        );
        let (updated, action) = apply_marker_block(&original, false).expect("should update");
        assert_eq!(action, "Upgraded");
        assert!(updated.contains("## Other"));
        assert!(updated.contains("foo"));
        assert!(!updated.contains("old block"));
        assert!(updated.contains(&format!("v={}", AGENTS_MD_VERSION)));
    }

    #[test]
    fn force_regenerates_same_version() {
        let original = format!(
            "{} v={} -->\nstale body\n{}\n",
            AGENTS_MARKER_START, AGENTS_MD_VERSION, AGENTS_MARKER_END
        );
        let (updated, action) = apply_marker_block(&original, true).expect("should update");
        assert_eq!(action, "Regenerated");
        assert!(!updated.contains("stale body"));
    }

    #[test]
    fn marker_block_roundtrips_through_find() {
        // marker_block() output must be parseable by find_marker_block() and
        // the version we wrote must match what we read back. Catches any
        // format drift between the two functions.
        let block = marker_block();
        let wrapped = format!("# Agent Instructions\n\nSome user content.\n\n{}\n", block);
        let (_s, _e, ver) = find_marker_block(&wrapped).expect("must locate marker");
        assert_eq!(ver, Some(AGENTS_MD_VERSION));
    }

    #[test]
    fn preserves_user_content_outside_markers() {
        let original = format!(
            "# Custom Header\n\n## Pre-existing section\n\nSome user notes.\n\n{} v=1 -->\nold\n{}\n\n## Post section\n\nMore notes.\n",
            AGENTS_MARKER_START, AGENTS_MARKER_END
        );
        let (updated, _) = apply_marker_block(&original, false).expect("should update");
        assert!(updated.contains("Some user notes"));
        assert!(updated.contains("More notes"));
        assert!(updated.contains("## Pre-existing section"));
        assert!(updated.contains("## Post section"));
    }
}
