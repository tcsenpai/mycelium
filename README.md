<p align="center">
  <img src="assets/logo.png" alt="Mycelium" width="120" height="120">
</p>

# Mycelium

**A task manager built for coding agents.** Mycelium gives an AI agent
(Claude Code, Cursor, Aider, or your own) a durable, git-trackable place to
plan and remember work across sessions. State lives in a SQLite file inside
your repo, so when an agent's context is cleared, a session ends, or a
teammate pulls the branch three days later, the plan is still there: the same
epics, tasks, dependencies, and open follow-ups. The agent reconstructs where
it left off from `myc list`, not from your memory.

It is a one-shot, scriptable CLI with JSON output and a zero-config data model,
which is exactly what an agent needs to drive it reliably. Humans get the same
tool (plus an optional desktop GUI, [MycUI](#gui-mycui-pre-built-download)).

[![crates.io](https://img.shields.io/crates/v/mycelium-manager.svg)](https://crates.io/crates/mycelium-manager)
[![docs.rs](https://img.shields.io/docsrs/mycelium-manager)](https://docs.rs/mycelium-manager)
[![Downloads](https://img.shields.io/crates/d/mycelium-manager.svg)](https://crates.io/crates/mycelium-manager)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

<p align="center">
  <img src="assets/demo.gif" alt="A coding agent using Mycelium to plan and track work" width="900">
  <br>
  <em>A real Claude Code session driving <code>myc</code> to plan work in a repo.</em>
</p>

## Why agents

- **State that outlives the session**: the plan is a file in the repo
  (`.mycelium/mycelium.db`), not the agent's context window. Clear the context,
  start a new session, or hand the branch to another machine, and the epics,
  tasks, dependencies, and follow-ups are still intact.
- **Git is the sync layer**: commit `.mycelium/` and the plan travels with the
  code. A teammate (or agent) who pulls the branch inherits the exact same
  task graph. No server, no account, no external service.
- **One-shot and scriptable**: every command is a single non-interactive
  invocation with `--format json` and `--quiet` for clean parsing. An agent
  drives it without a REPL or a session to manage.
- **A drop-in agent contract**: `myc init` writes an `AGENTS.md` describing the
  workflow, so the agent knows how to use the tool without you explaining it.
  Follow-ups let it jot "oh-by-the-way" findings mid-task without derailing.
- **Blocking that means something**: dependency links with cycle detection, so
  an agent can't close a task whose blockers are still open.

## How it works in practice

A walkthrough of a real agent session in a repo.

**1. Setup: the agent gets its instructions.** Running `myc init` (once, per
project) writes an `AGENTS.md` into the repo describing the whole workflow:
the commands, the data model, and the rules (e.g. "every task belongs to an
epic", "surface open follow-ups before wrapping up"). Agents like Claude Code
read `AGENTS.md` automatically, so the tool is self-documenting and you never
have to explain `myc` to the agent.

**2. During work: the agent tracks as it goes.** You ask the agent to build a
feature. It decomposes the work into the plan, right in the repo:

```bash
myc epic create --title "Auth" --description "Login + sessions"
myc task create --title "Password login" --epic 1 --priority high
myc task create --title "Session middleware" --epic 1
myc task link blocks --task 2 3       # sessions depend on login
myc task update 2 --status in_progress
```

Mid-task it notices something unrelated and jots it down without losing focus,
using the lightweight **follow-ups** scratchpad:

```bash
myc followup add "TODO: rotate the JWT secret, it's hardcoded in config.rs"
```

**3. Wrap-up: nothing gets silently dropped.** When the agent finishes, a
[Claude Code](https://docs.claude.com/en/docs/claude-code) **Stop hook**
(installed automatically by `myc init`) fires and checks for open follow-ups.
If any exist, it feeds them back to the agent, which surfaces them to you
instead of ending the turn as if everything were done:

> Before we wrap, 1 open follow-up: "rotate the JWT secret, hardcoded in
> config.rs". Want me to handle it now, or leave it for later?

The hook self-gates: it stays silent outside mycelium projects and only fires
when there's actually something open, so it never nags.

**4. Next session: the state is still there.** Days later, a fresh agent
session (cleared context, or a teammate on another machine who pulled the
branch) starts by reading the plan back, not by asking you what happened:

```bash
myc list                 # the Auth epic, its tasks, "Session middleware [blocked by T2]"
myc followup list -o     # the JWT-secret note is still waiting
```

It resumes exactly where the last session left off. The plan lived in
`.mycelium/mycelium.db` and travelled with the code through git.

## Features

- **Agent-Optimized**: One-shot CLI, `--format json`, `--quiet`, `AGENTS.md` contract
- **State Persistence**: Plan survives context resets; reconstructable from `myc list`
- **Git-Trackable**: SQLite storage designed for version control and branch sync
- **Dependency Management**: Task blocking with cycle detection
- **Category-Prefixed IDs**: Displays `E3`/`T3`/`F3` so epic/task/follow-up IDs never get confused; input still accepts bare numbers
- **Follow-ups**: Lightweight scratch table for non-blocking items captured mid-work
- **Smart List View**: Tree visualization for dependencies, epic grouping for simple lists
- **Assignees**: Local assignees with GitHub username linking
- **External References**: Link tasks to GitHub issues/PRs and URLs
- **Task Notes**: Add comments and notes to tasks
- **Task Cloning**: Duplicate tasks with all metadata
- **Batch Operations**: Close, tag, or move multiple tasks at once
- **Single Binary**: Statically compiled, no dependencies
- **Fast**: Sub-100ms response time for typical operations
- **Safe**: Comprehensive error handling and validation

## Installation

### CLI (`myc`) via crates.io (recommended)

```bash
cargo install mycelium-manager
```

Installs the `myc` binary to `~/.cargo/bin/`. Requires Rust 1.75+.

#### Updating

```bash
myc update
```

Updates the binary via `cargo install --force`, then resyncs this project's
`AGENTS.md` and follow-up hook to the new version. If `cargo` isn't available
it skips the binary step and just resyncs the artifacts (update the binary by
hand, then rerun).

### CLI (`myc`) from source

```bash
git clone https://github.com/tcsenpai/mycelium
cd mycelium
cargo build --release
# Binary will be at target/release/myc
sudo cp target/release/myc /usr/local/bin/
```

### GUI (`MycUI`) pre-built download

Each tagged release ships desktop bundles on the
[GitHub Releases](https://github.com/tcsenpai/mycelium/releases) page:
`.dmg`/`.app` (macOS), `.deb`/`.AppImage` (Linux), and `.msi`/`.exe` (Windows).
Download the one for your platform and install it. Every release also attaches
the `myc` CLI binaries, alongside the crates.io publish.

> Note: macOS bundles are currently unsigned, so Gatekeeper may warn on first
> launch (right-click → Open to bypass).

### GUI (`MycUI`) from source

MycUI is a [Tauri](https://tauri.app) desktop app built with React and TypeScript.

**Prerequisites**: [Rust](https://rustup.rs), [Bun](https://bun.sh), and [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform.

```bash
cd mycui
bun install
bun run tauri:build
```

The built app will be in `mycui/src-tauri/target/release/bundle/` with platform-specific installers (`.deb`, `.AppImage`, `.dmg`, `.msi`).

For development:

```bash
cd mycui
bun install
bun run tauri:dev
```

### One-Line Install (Linux & macOS)

```bash
git clone https://github.com/tcsenpai/mycelium && cd mycelium && ./install.sh
```

The install script detects your platform, builds, and installs both `myc` (CLI) and `MycUI` (GUI). On macOS, MycUI is installed as `/Applications/MycUI.app`. On Linux, both binaries go to `/usr/local/bin/`.

```bash
./install.sh --cli       # Install only the CLI
./install.sh --gui       # Install only MycUI
./install.sh --all       # Install both (default)
INSTALL_DIR=~/.local/bin ./install.sh --cli  # Custom install path (CLI)
```

## Versioning

The CLI (`myc`) and the desktop app (MycUI) are versioned independently and
released on their own cadence. The CLI is published to crates.io as
`mycelium-manager`; MycUI ships as a desktop bundle attached to GitHub
Releases. There is no requirement to run matching version numbers.

They stay compatible because both are built on the same `mycelium-core` crate,
so they share one schema, one set of migrations, and one data layer. You can
run any released CLI alongside any released MycUI against the same project, use
either tool on its own, or mix them freely. Neither depends on the other at
runtime: the CLI is a standalone binary, and MycUI talks to the database
directly through core rather than shelling out to `myc`.

## Quick Start

```bash
# Initialize a new mycelium project
myc init

# Create an epic
myc epic create --title "Feature X" --description "Build feature X"

# Create tasks
myc task create --title "Design API" --description "Define the API surface and contracts" --epic 1 --priority high
myc task create --title "Implement backend" --description "Build the backend services and persistence layer" --epic 1 --priority critical --due 2025-06-01

# Set up dependencies (task 1 blocks task 2)
myc task link blocks --task 1 2

# View dependency tree
myc deps show 2

# Close tasks (blocked tasks prevent closing)
myc task close 1
myc task close 2

# Batch operations
myc task batch-op close 3 4 5              # Close multiple tasks
myc task batch-op tag urgent 1 2 3         # Tag multiple tasks
myc task batch-op move 2 6 7 8             # Move tasks to epic #2

# Task notes
myc task note 1 "Found edge case with auth"
myc task notes 1                           # View notes

# Clone a task
myc task clone 1 --title "API Design v2"

# View project summary
myc summary
```

## Commands

### Project

```bash
myc init                    # Initialize mycelium (also installs the follow-up hook)
myc init --no-hooks         # ...without installing the hook
myc update                  # Update the binary + resync AGENTS.md and the hook
myc summary                 # Show project overview
myc doctor                  # Check system health and configuration
```

> **IDs are category-prefixed.** Commands display `E3` (epic), `T3` (task),
> `F3` (follow-up). You can pass either the bare number (`myc task show 3`) or
> the prefixed form (`myc task show T3`, case-insensitive); a wrong-category
> prefix (`myc task show E3`) is rejected with a hint. JSON/CSV keep raw
> integer IDs.

### Epics

```bash
myc epic create --title "..." [--description "..."]
myc epic list               # List all epics with task counts
myc epic show <id>          # Show epic details with tasks
myc epic update <id> [--title "..."] [--description "..."] [--status open|closed]
myc epic delete <id> [--force]
```

### Tasks

```bash
myc task create --title "..." [options]
  --epic <id>               # Assign to epic
  --priority <low|medium|high|critical>
  --assignee <id>           # Assign to person
  --due <YYYY-MM-DD>        # Set due date

myc list [filters]          # List tasks and epics (shows tree view if dependencies exist)
  --epic <id>               # Filter by epic
  --status <open|closed>    # Filter by status (defaults to 'open')
  --priority <level>        # Filter by priority
  --assignee <id>           # Filter by assignee
  --blocked                 # Show only blocked tasks
  --overdue                 # Show only overdue tasks
  --all                     # Show all tasks including closed

myc task list [filters]     # Same as myc list, but task-specific

myc task show <id>          # Show task details
myc task update <id> [options]
myc task close <id> [--force]   # Close (blocked tasks need --force)
myc task reopen <id>
myc task delete <id> [--force]
myc task assign <task_id> <assignee_id|0>
```

### Batch Operations

```bash
# Close multiple tasks at once
myc task batch-op close <id> [<id>...] [--force]

# Add a tag to multiple tasks
myc task batch-op tag <tag> <id> [<id>...]

# Move multiple tasks to an epic (use 0 for no epic)
myc task batch-op move <epic_id> <id> [<id>...]
```

### Task Notes

```bash
myc task note <task_id> "Note content"    # Add a note to a task
myc task notes <task_id>                  # Show all notes for a task
```

### Task Cloning

```bash
myc task clone <id> [--title "New Title"]  # Clone a task (copies description, priority, etc.)
```

### Dependencies

```bash
myc task link blocks --task <blocker_id> <blocked_id>
myc deps show <task_id>     # Show dependency tree
myc deps unlink <task_id> <blocked_task_id>
```

### Assignees

```bash
myc assignee create --name "..." [--email "..."] [--github "username"]
myc assignee list           # List with task counts
myc assignee show <id>
myc assignee delete <id> [--force]
```

### External References

```bash
myc task link github-issue --task <id> "owner/repo#123"
myc task link github-pr --task <id> "owner/repo#456"
myc task link url --task <id> "https://..."
myc task unlink <ref_id>
```

### Follow-ups

Lightweight scratch table for non-blocking items captured mid-work
(bugs you noticed, questions, ideas). They are separate from tasks and
carry no epic, priority, or dependencies. Body is required, title optional. Statuses:
`open`, `in_progress`, `done`, `wontfix`.

```bash
myc followup add "body text" [--title "tag"]    # capture
myc fu add "short form alias works"

myc followup list                               # all items (default, alias -a)
myc followup list -o                            # only active (open + in_progress)
myc followup list -c                            # only closed (done + wontfix)
myc followup list --status done                 # exact status bucket
myc followup show <id>
myc followup next                               # lowest-ID active

myc followup start <id>                         # → in_progress
myc followup done <id> [--reason "..."]
myc followup wontfix <id> [--reason "..."]
myc followup reopen <id>

myc followup edit <id> --body "new body" [--title -|"new title"]
myc followup append <id> "more context"         # timestamped, additive
myc followup rm <id> [--force]
myc followup promote <id> [--epic N] [--priority high]   # convert to task
myc followup count                              # JSON: {open, in_progress, done, wontfix}
```

After `myc task close`, mycelium prints a one-line reminder if any
active follow-ups exist. Agents using mycelium MUST run `myc followup
list` at the end of every work unit and surface open items to the user
before wrapping (see AGENTS.md).

### Hooks

```bash
myc hooks install            # install the follow-up Stop hook into .claude/ (local)
myc hooks install --global   # ...into ~/.claude/ instead
myc hooks uninstall          # remove it (add --global for ~/.claude)
myc hooks status             # show where it's installed
```

### Reporting & Export

```bash
myc summary                 # Project overview
myc export json [--output file.json]
myc export csv [--output file.csv]
```

## Global Options

```bash
--format <table|json>       # Output format (default: table)
--quiet                     # Suppress non-error output
--help                      # Show help
--version                   # Show version
```

## Data Model

```
Epic
├── id, title, description
├── status (open/closed)
└── Tasks[]

Task
├── id, title, description (optional)
├── status (open/closed), priority (low/medium/high/critical)
├── epic_id (optional), assignee_id (optional)
├── due_date (optional)
├── dependencies (blocks/blocked_by)
└── external_refs (GitHub issues/PRs, URLs)
```

## Git Integration

Mycelium stores data in `.mycelium/mycelium.db` using SQLite with WAL mode. This makes it git-trackable:

```bash
# Add to your repo
git add .mycelium/
git commit -m "Add mycelium project tracking"

# The .mycelium/.gitignore excludes WAL files
```

## For AI Agents

Mycelium is optimized for agentic workflows.

**The resume pattern.** At the start of a task, the agent reads its own
prior plan instead of relying on the context window:

```bash
myc list --format json        # what's the current task graph?
myc task list --blocked       # what's stuck?
myc followup list -o          # any open "oh-by-the-way" items from last time?
```

At the end, it records state that will still be there next session:

```bash
myc task update 4 --status closed
myc followup add "auth tests are flaky under load, investigate"
```

Across a context reset or a fresh session on the same branch, the plan is
unchanged and the agent picks up exactly where it left off.

**Everyday commands:**

```bash
# Use --quiet to get just IDs
myc task create --title "New task" --quiet  # outputs: 42

# Include a description when useful
myc task create --title "New task" --description "Explain the work item"

# Use --format json for parsing
myc task list --format json

# Check blocked tasks
myc task list --blocked

# Export for analysis
myc export json
```

### Claude Code follow-up hook

A [Claude Code](https://docs.claude.com/en/docs/claude-code) Stop hook enforces
the end-of-task follow-up check, so it no longer relies on the agent remembering
the AGENTS.md rule. The hook script is embedded in the `myc` binary, so it ships
with `cargo install` — no extra files to fetch.

`myc init` installs it automatically into the project's `.claude/` (commit that
directory so the whole team gets the check). Manage it explicitly with:

```bash
myc hooks install            # project-local .claude/ (default)
myc hooks install --global   # ~/.claude/ instead
myc hooks uninstall          # remove (add --global for ~/.claude)
myc hooks status             # show where it's installed
myc init --no-hooks          # initialize without installing the hook
```

The hook self-gates: it stays silent outside mycelium projects (detected via the
`myc:agents-start` marker in `AGENTS.md`) and only fires when **open** follow-ups
exist. It also self-dedups on the session, so a global and a project-local copy
can coexist without firing the check twice. Requires `jq`.

> Prefer a shell installer for a global-only setup? `hooks/install-hook.sh`
> (and `--uninstall`) still installs into `~/.claude/` without the binary.

## Configuration

No configuration needed! All data is stored in the project-local `.mycelium/` directory.

## Safety Features

- **Atomic operations**: Database transactions ensure data integrity
- **Dependency validation**: Circular dependencies are prevented
- **Blocker checks**: Tasks with open blockers cannot be closed (without `--force`)
- **Confirmation prompts**: Destructive operations require `--force` or user confirmation
- **Clear errors**: All errors include actionable guidance

## Performance

- Sub-100ms response time for typical operations
- SQLite with proper indexing
- WAL mode for concurrent read/write
- Single binary, no runtime dependencies

## Development

```bash
# Clone
git clone https://github.com/tcsenpai/mycelium
cd mycelium

# Build & test the CLI
cargo build --release
cargo test
cargo run -- init

# Run MycUI in dev mode
cd mycui
bun install
bun run tauri:dev
```

## Architecture

- **Rust** - Type-safe, performant, single binary
- **mycelium-core** - Shared crate (db, models, migrations, errors) consumed by both the CLI and MycUI, so they use one schema and data layer
- **SQLite** - Embedded, git-trackable, ACID-compliant
- **Clap** - Command-line parsing with derive macros
- **Rusqlite** - SQLite bindings with bundled lib
- **Tauri** - Desktop GUI framework (MycUI)
- **React + TypeScript** - MycUI frontend
- **Hand-written CSS** - MycUI styling (no CSS framework)

## License

MIT License - see [LICENSE](LICENSE) file.

## Contributing

Contributions welcome! Feel free to open issues and pull requests.

## Acknowledgments

Inspired by [beads](https://github.com/qsantos/beads) and the need for a robust, git-trackable task manager that works seamlessly with AI agents.
