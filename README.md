# Mycelium 🍄

A robust, production-grade task/plan manager CLI designed for reliability, agent usage, and git-trackable project management.

[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

## Features

- **📦 Single Binary**: Statically compiled, no dependencies
- **🗄️ Git-Trackable**: SQLite storage designed for version control
- **🔗 Dependency Management**: Task blocking with cycle detection
- **👤 Assignees**: Local assignees with GitHub username linking
- **🌐 External References**: Link tasks to GitHub issues/PRs and URLs
- **🤖 Agent-Optimized**: One-shot CLI with JSON output support
- **⚡ Fast**: Sub-100ms response time for typical operations
- **🛡️ Safe**: Comprehensive error handling and validation
- **📋 Smart List View**: Tree visualization for dependencies, epic grouping for simple lists
- **📝 Task Notes**: Add comments and notes to tasks
- **📎 Task Cloning**: Duplicate tasks with all metadata
- **📦 Batch Operations**: Close, tag, or move multiple tasks at once

## Installation

### CLI (`myc`) — From Source

```bash
git clone https://github.com/tcsenpai/mycelium
cd mycelium
cargo build --release
# Binary will be at target/release/myc
sudo cp target/release/myc /usr/local/bin/
```

### GUI (`MycUI`) — From Source

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
myc init                    # Initialize mycelium in current directory
myc summary                 # Show project overview
myc doctor                  # Check system health and configuration
```

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

Mycelium is optimized for agentic workflows:

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
- **SQLite** - Embedded, git-trackable, ACID-compliant
- **Clap** - Command-line parsing with derive macros
- **Rusqlite** - SQLite bindings with bundled lib
- **Tauri** - Desktop GUI framework (MycUI)
- **React + TypeScript** - MycUI frontend
- **Tailwind CSS** - MycUI styling

## License

MIT License - see [LICENSE](LICENSE) file.

## Contributing

Contributions welcome! Feel free to open issues and pull requests.

## Acknowledgments

Inspired by [beads](https://github.com/qsantos/beads) and the need for a robust, git-trackable task manager that works seamlessly with AI agents.
