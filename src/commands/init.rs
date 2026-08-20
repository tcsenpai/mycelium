use crate::commands::{confirm_default_yes, INFO_PREFIX, SUCCESS_PREFIX, WARNING_PREFIX};
use crate::db::Database;
use crate::error::Result;
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Bump this whenever AGENTS_MD_CONTENT changes. `myc prime-agents`
/// without --force only updates when the embedded marker version differs.
const AGENTS_MD_VERSION: u32 = 12;
const AGENTS_MARKER_START: &str = "<!-- myc:agents-start";
const AGENTS_MARKER_END: &str = "<!-- myc:agents-end -->";

const AGENTS_MD_CONTENT: &str = r#"
## Project Management with Mycelium

This project uses [Mycelium](https://github.com/tcsenpai/mycelium) (`myc`) for task and epic management.

### Quick Reference

```bash
# Initialize mycelium in this project (creates .mycelium/ directory).
# Commands resolve the nearest .mycelium/ by walking UP from the cwd, so run
# them from anywhere in the project. Running `myc init` inside a subdir of an
# existing project warns and asks (default yes) before creating a SEPARATE
# nested project there; `myc init --force` creates it without asking.
myc init

# Create an epic (a large body of work)
myc epic create --title "Feature X" --description "Build feature X"

# Create tasks within an epic
myc task create --title "Implement Y" --description "Build the implementation for Y" --epic 1 --priority high --due 2025-12-31

# Task priorities: low, medium, high, critical
# Task status: open, in_progress, closed
# Mark a task as in progress (there is no `task start`; use update):
myc task update 1 --status in_progress

# List tasks. `myc list` (top-level) shows a TREE with dependencies and epic
# grouping — use it to see the overall state. `myc task list` is a flat list.
myc list
myc task list
myc task list --epic 1
myc task list --overdue
myc task list --blocked
myc task list --all          # include closed tasks
myc task list --tree         # parent > child hierarchy
myc task list --parent 5     # flat list of the direct children of task 5
myc task list --parent 0     # only top-level tasks (no parent)
myc task list --parent 5 --tag sp   # children of 5 filtered by tag (combinable)

# Find a task id from a title/description fragment (searches both, case-insensitive).
# Quiet mode is grep-friendly: "<id>\t<title>" per line. JSON gives full objects.
myc task search "custom_field"
myc -q task search "subquery"        # id + title, tab-separated
myc --format json task search "SP"   # full task objects

# Subtasks (parent/child hierarchy — distinct from epics and dependencies).
# Group a family of tasks under a "hat" task without inventing an epic.
myc task create -t "child task" --parent 1
myc task update 2 --parent 1   # re-parent existing task (use 0 to detach)

# Manage dependencies (task 1 blocks task 2)
myc task link blocks --task 1 2
myc deps show 2

# Non-blocking references between tasks (relates / duplicate). Symmetric,
# never block or close anything — just mark "these are related / the same".
myc task link relates 1 2
myc task link duplicate 1 2
myc task refs 1               # list a task's references
myc task ref-unlink 1 2 relates

# Close tasks (blocked tasks cannot be closed without --force).
# A parent with open subtasks prompts; --cascade closes the whole subtree.
myc task close 1
myc task close 1 --cascade

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
- **Subtask**: A task with a `parent` task (hierarchy). Distinct from epics (one-level grouping) and dependencies (blocking). Closing a parent never auto-closes children.
- **Dependency**: Task A blocks Task B (B cannot close until A is closed)
- **Reference**: A non-blocking, symmetric link between two tasks — `relates` (same family) or `duplicate` (same thing). Marks only; never blocks or closes.
- **Assignee**: Person assigned to a task (can have GitHub username)
- **External Ref**: Link to GitHub issues/PRs or URLs

### ID Prefixes (v5)

Each entity has its **own** integer sequence, so a bare number is ambiguous
across categories. Mycelium now **displays** IDs with a one-letter category
prefix so they can't be confused:

| Category | Prefix | Example |
|---|---|---|
| Epic | `E` | `E3` |
| Task | `T` | `T3` |
| Follow-up | `F` | `F3` |
| Assignee | `A` | `A3` |
| External ref | `R` | `R3` |

**Input is backward compatible.** Every command still accepts a bare integer
(`myc task show 3`) *and* the prefixed form (`myc task show T3`,
case-insensitive). Passing the **wrong** category prefix is a hard error with a
hint — e.g. `myc task show E3` tells you `E3` is an epic and suggests
`myc epic show E3`. This catches copy/paste mix-ups.

`--format json` output is unchanged: the `id` field stays a raw integer, so
existing scripts and the Linear sync keep working.

### Git Tracking

The `.mycelium/` directory contains the SQLite database and should be committed to git:

```bash
git add .mycelium/
git commit -m "Add mycelium project tracking"
```

### Follow-up Stop hook (Claude Code)

`myc init` installs a project-local Claude Code Stop hook into `.claude/`
(script + `settings.json` wiring) that enforces the end-of-task follow-up
check automatically. Commit `.claude/` so the whole team gets it.

```bash
myc init --no-hooks          # skip the hook install
myc init --force             # create a nested project in a subdir without asking
myc hooks install            # (re)install into the project's .claude/
myc hooks install --global   # install into ~/.claude instead
myc hooks uninstall          # remove (add --global for ~/.claude)
myc hooks status             # show where it's installed
```

The hook self-dedups, so a global and a local copy can coexist without
firing the check twice.

### Updating

```bash
myc update   # cargo install --force, then resync AGENTS.md + hook to the new version
```

`myc update` updates the binary via cargo, then re-runs `prime-agents --force`
and `hooks install` so this project's AGENTS.md and hook match the new version.
If cargo isn't available it skips the binary step and just resyncs the
artifacts (update the binary by hand, then rerun).

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
myc followup snooze [--turns N]             # silence the Stop hook for N stops (default 5)
myc followup snooze --turns 0               # clear an active snooze
```

**Agent rule — end-of-task follow-up check** (MANDATORY)

At the end of every mycelium-tracked unit of work (closing a task,
finishing a user-requested change that touched myc state), the agent
MUST:

1. Run `myc followup list --format json` (or `myc followup count
   --format json`).
2. If `open > 0`, surface those to the user before wrapping:
   > "Before we wrap — N open follow-up(s): [titles/bodies]. Want me to
   > handle any now, or leave for later?"
   Count only `open`, not `active`: `in_progress` items are already
   being worked and don't need an end-of-task decision.
3. **Never silently process them.** Always ask.

`myc task close` itself also prints a one-line reminder (only for open
follow-ups older than ~60s; fresh or in_progress ones stay silent), but
the agent should still proactively check.

The Stop hook re-runs this check on **every** stop while `open > 0`. Once
you've surfaced the follow-ups to the user (step 2) and they've chosen to
leave them for later, run `myc followup snooze` to silence the hook for the
next few stops instead of being re-prompted each turn. Snooze is
project-scoped and consumes one stop at a time.

Use `myc followup add` during work to capture anything you notice but
shouldn't act on right now.

### For AI Agents

When working on this project:

1. At the START of a task, reconstruct state instead of relying on memory:
   `myc list` (tree with dependencies) and `myc followup list -o` (open items).
2. Check blocked tasks: `myc task list --blocked`
3. Create tasks for new work: `myc task create --title "..." --description "..." --epic N`
4. Mark a task in progress while you work on it: `myc task update N --status in_progress`
5. Capture incidental observations as follow-ups: `myc followup add "..."`
6. At end of task: `myc followup list` and surface open ones to the user
7. Mark tasks complete when done: `myc task close N`
8. Use `--format json` for machine-readable output: `myc task list --format json`

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
/// Returns true if the database was just created, false if it already existed.
///
/// The DB file — not the directory — is the source of truth for "initialized".
/// A dir that exists without a db file (partial/interrupted init, or the db was
/// deleted) is repaired: the db is created and the dir backfilled as needed.
fn ensure_project_initialized(mycelium_dir: &Path) -> Result<bool> {
    let db_path = mycelium_dir.join("mycelium.db");
    if db_path.exists() {
        return Ok(false);
    }

    fs::create_dir_all(mycelium_dir)?;

    let gitignore_path = mycelium_dir.join(".gitignore");
    if !gitignore_path.exists() {
        let gitignore_content = r#"# Mycelium database
# The database file is git-trackable but WAL files are not
*.db-wal
*.db-shm
# Temporary files
*.tmp
# Follow-up Stop-hook snooze counter (per-session, project-local)
.followup-snooze
"#;
        fs::write(&gitignore_path, gitignore_content)?;
    }

    Database::open(&db_path)?;

    println!("{} Mycelium project initialized", SUCCESS_PREFIX.green());
    println!("  Database: {}", db_path.display());
    println!("  Git tracking: Add {} to your repo", ".mycelium/".cyan());

    Ok(true)
}

pub fn execute(force_init: bool, no_hooks: bool) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mycelium_dir = cwd.join(".mycelium");
    let agents_md_path = cwd.join("AGENTS.md");

    // Two distinct "already there" cases, resolved separately:
    //   (a) THIS dir is already a project (cwd/.mycelium/mycelium.db exists) —
    //       nothing to do, don't re-init over yourself.
    //   (b) an ANCESTOR dir is a project (the walk finds a db higher up) but the
    //       cwd is not — running `init` here means the user wants a project HERE.
    //       We used to silently return "already initialized" (no-nesting), which
    //       left the user in a subdir with no local .mycelium/ and no explanation.
    //       Now we WARN with the ancestor path and ask (default yes) before
    //       creating a nested local project.
    let local_db = cwd.join(".mycelium").join("mycelium.db");
    if local_db.exists() && !force_init {
        println!(
            "{} Mycelium project already initialized here ({})",
            INFO_PREFIX.blue(),
            display_path(&mycelium_dir)
        );
        install_hook_if_wanted(no_hooks);
        return Ok(());
    }

    if !local_db.exists() && !force_init {
        let ancestor_db = crate::commands::get_db_path();
        if ancestor_db.exists() {
            // ancestor_db is guaranteed NOT in cwd here (local_db doesn't exist).
            let ancestor_dir = ancestor_db
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| ancestor_db.clone());
            println!(
                "{} A Mycelium project already exists in a parent directory ({}).",
                WARNING_PREFIX.yellow(),
                display_path(&ancestor_dir)
            );
            // Default YES: creating a local project here is the natural intent of
            // running `init` in this dir. Non-interactive (no TTY / scripted)
            // proceeds with the default rather than blocking.
            if !confirm_default_yes("Create a separate Mycelium project in the current directory?")
            {
                println!(
                    "{} Aborted — using the parent project. Run commands from here; they resolve upward.",
                    INFO_PREFIX.blue()
                );
                return Ok(());
            }
        }
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

    install_hook_if_wanted(no_hooks);

    Ok(())
}

/// Install the project-local follow-up hook unless the user opted out. Non-fatal:
/// a failure here warns but does not fail `init`.
fn install_hook_if_wanted(no_hooks: bool) {
    if no_hooks {
        println!(
            "{} Skipped follow-up hook install (--no-hooks)",
            INFO_PREFIX.blue()
        );
        return;
    }
    if let Err(e) = crate::commands::hooks::install(crate::commands::hooks::Scope::Local) {
        println!(
            "{} Could not install follow-up hook: {} (run `myc hooks install` later)",
            INFO_PREFIX.blue(),
            e
        );
    }
}

pub fn execute_prime_agents(force: bool, path: Option<&Path>) -> Result<()> {
    // Resolve the real project via the same ancestor walk every other command
    // uses, so `myc prime-agents` from a subdir targets the repo's AGENTS.md
    // (and .mycelium/), not a phantom one in the cwd. get_mycelium_dir() falls
    // back to cwd/.mycelium when no ancestor project exists, preserving the
    // "prime-agents in a fresh dir creates it here" behavior.
    let mycelium_dir = crate::commands::get_mycelium_dir();
    let project_root = mycelium_dir
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let agents_md_path = path
        .map(|p| project_root.join(p))
        .unwrap_or_else(|| project_root.join("AGENTS.md"));

    ensure_project_initialized(&mycelium_dir)?;

    // AGENTS.md is resolved against the PROJECT ROOT (parent of the ancestor
    // .mycelium/), which is intentional — one AGENTS.md per project, not one
    // per subdir. But when the command runs from a subdir, "wrote AGENTS.md"
    // with no path is misleading (looks like it landed in the cwd). Always show
    // the resolved path so a subdir caller isn't left hunting a phantom file.
    let shown_path = display_path(&agents_md_path);

    if !agents_md_path.exists() {
        fs::write(
            &agents_md_path,
            format!("# Agent Instructions\n{}", marker_block()),
        )?;
        println!(
            "{} Created {} with mycelium instructions (v{})",
            SUCCESS_PREFIX.green(),
            shown_path,
            AGENTS_MD_VERSION
        );
        return Ok(());
    }

    let existing = fs::read_to_string(&agents_md_path)?;
    match apply_marker_block(&existing, force) {
        Some((updated, action)) => {
            fs::write(&agents_md_path, updated)?;
            println!(
                "{} {} mycelium block in {} (v{})",
                SUCCESS_PREFIX.green(),
                action,
                shown_path,
                AGENTS_MD_VERSION
            );
        }
        None => {
            println!(
                "{} {} mycelium block already at v{} — no change (use --force to regenerate)",
                INFO_PREFIX.blue(),
                shown_path,
                AGENTS_MD_VERSION
            );
        }
    }

    Ok(())
}

/// Render a path for user output: absolute if we can canonicalize it (so a
/// subdir caller sees WHERE the project-root AGENTS.md actually is), otherwise
/// the path as-is. Canonicalize can fail on a not-yet-created file, so fall
/// back to the parent dir + filename when possible.
fn display_path(p: &Path) -> String {
    if let Ok(abs) = p.canonicalize() {
        return abs.display().to_string();
    }
    // File may not exist yet (create path): canonicalize the parent instead.
    if let (Some(parent), Some(name)) = (p.parent(), p.file_name()) {
        if let Ok(abs_parent) = parent.canonicalize() {
            return abs_parent.join(name).display().to_string();
        }
    }
    p.display().to_string()
}

/// True when the project's `AGENTS.md` has a mycelium marker block whose
/// embedded version differs from this binary's `AGENTS_MD_VERSION`. False when
/// the file is missing, has no marker, or is already current — the auto-refresh
/// only acts on a present-but-outdated block (never creates AGENTS.md
/// unprompted). Resolves AGENTS.md against the real project root (ancestor
/// walk), so it works from any subdirectory — matching maybe_refresh's own
/// project detection.
pub fn is_agents_block_stale() -> bool {
    let project_root = crate::commands::get_mycelium_dir();
    let agents_md_path = match project_root.parent() {
        Some(root) => root.join("AGENTS.md"),
        None => return false,
    };
    let Ok(content) = fs::read_to_string(&agents_md_path) else {
        return false;
    };
    matches!(find_marker_block(&content), Some((_, _, ver)) if ver != Some(AGENTS_MD_VERSION))
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
    fn ensure_init_repairs_dir_without_db() {
        // Regression: a .mycelium/ dir that exists but has no db file must be
        // treated as uninitialized and repaired, not reported "already done".
        let tmp = std::env::temp_dir().join(format!("myc-init-test-{}", std::process::id()));
        let mycelium_dir = tmp.join(".mycelium");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&mycelium_dir).unwrap();
        assert!(mycelium_dir.exists());
        assert!(!mycelium_dir.join("mycelium.db").exists());

        // dir present, db absent → must create the db and report "created".
        let created = ensure_project_initialized(&mycelium_dir).unwrap();
        assert!(created, "should create db when dir exists but db missing");
        assert!(mycelium_dir.join("mycelium.db").exists());

        // Second call is a no-op now that the db exists.
        let created_again = ensure_project_initialized(&mycelium_dir).unwrap();
        assert!(!created_again, "should be idempotent once db exists");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn display_path_is_absolute_for_existing_and_new_files() {
        // Regression: `prime-agents` from a subdir writes AGENTS.md at the
        // PROJECT ROOT (ancestor of .mycelium/), and the old message showed a
        // bare "AGENTS.md" with no path — so a subdir caller ran `ls AGENTS.md`
        // in the cwd, found nothing, and thought the command lied. display_path
        // must yield an ABSOLUTE path in both cases (file exists, and file to be
        // created) so the message tells the caller where it actually landed.
        let tmp = std::env::temp_dir().join(format!("myc-disp-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // Existing file.
        let existing = tmp.join("AGENTS.md");
        fs::write(&existing, "x").unwrap();
        let shown = display_path(&existing);
        assert!(
            Path::new(&shown).is_absolute(),
            "existing-file path must be absolute, got {shown}"
        );

        // Not-yet-created file (the create branch): parent exists, file doesn't.
        let to_create = tmp.join("NEW_AGENTS.md");
        assert!(!to_create.exists());
        let shown_new = display_path(&to_create);
        assert!(
            Path::new(&shown_new).is_absolute(),
            "new-file path must be absolute, got {shown_new}"
        );
        assert!(shown_new.ends_with("NEW_AGENTS.md"));

        let _ = fs::remove_dir_all(&tmp);
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
