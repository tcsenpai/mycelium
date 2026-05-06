# Project Index: mycelium

Generated: 2026-05-06

## Project Structure

```
mycelium/
├── src/
│   ├── main.rs              # CLI entry point (myc binary)
│   ├── cli/mod.rs           # Clap CLI structure
│   ├── commands/            # Command implementations
│   │   ├── task.rs          # Task CRUD, linking, notes, cloning
│   │   ├── epic.rs          # Epic CRUD, notes
│   │   ├── assignee.rs      # Assignee management
│   │   ├── deps.rs          # Dependency management
│   │   ├── list.rs          # List epics/tasks
│   │   ├── summary.rs       # Project summary
│   │   ├── export.rs        # JSON/CSV export
│   │   ├── doctor.rs        # Health check
│   │   ├── init.rs          # Project initialization
│   │   └── linear.rs        # Linear API sync
│   ├── models/              # Data models
│   │   ├── task.rs, epic.rs, task_note.rs, epic_note.rs
│   │   ├── assignee.rs, dependency.rs, external_ref.rs
│   │   └── mod.rs
│   ├── linear/              # Linear API client
│   │   ├── client.rs, config.rs, sync.rs, mapping.rs
│   │   └── mod.rs
│   ├── db/                  # SQLite operations
│   │   ├── mod.rs, migrations.rs
│   │   └── mycelium.db (git-trackable)
│   └── error/mod.rs         # Error handling
├── mycui/                   # Tauri desktop app (MycUI)
│   ├── src/                 # React frontend
│   │   ├── App.tsx, main.tsx, index.css
│   │   └── lib/api.ts, types.ts
│   └── src-tauri/           # Rust backend
│       └── src/main.rs, db.rs, models.rs
├── Cargo.toml               # Rust dependencies
└── AGENTS.md               # Agent instructions
```

## Entry Points

- **CLI**: `src/main.rs` → `myc` binary
- **GUI**: `mycui/` → Tauri app (MycUI)
- **Tests**: `tests/integration_tests.rs`

## Core Modules

### Module: commands::task
- Path: `src/commands/task.rs`
- Exports: create, list, show, update, delete, close, reopen, clone_task, assign, link_github_issue, link_blocks, add_note, batch_close, batch_tag, batch_move
- Purpose: Task CRUD, dependencies, notes, batch operations

### Module: commands::epic
- Path: `src/commands/epic.rs`
- Exports: create, list, show, update, delete, add_note, show_notes
- Purpose: Epic CRUD with notes support

### Module: models::task
- Path: `src/models/task.rs`
- Exports: Task struct with id, title, description, status, priority, epic_id, assignee_id, due_date, tags
- Purpose: Task data model

### Module: linear
- Path: `src/linear/mod.rs`
- Exports: client, config, sync, mapping
- Purpose: Linear API integration for bidirectional sync

### Module: db
- Path: `src/db/mod.rs`
- Exports: init_db, get_connection, run_migrations
- Purpose: SQLite database operations with WAL mode

## Configuration

- `Cargo.toml`: Rust dependencies (clap, rusqlite, serde, chrono, reqwest)
- `mycui/package.json`: Node dependencies (React, Tauri, Zustand, TanStack Query)

## Dependencies

- **clap 4.5**: CLI argument parsing with derive macros
- **rusqlite 0.32**: SQLite bindings with bundled lib
- **serde 1.0**: Serialization/deserialization
- **chrono 0.4**: Date/time handling
- **reqwest 0.12**: HTTP client for Linear API
- **anyhow/thiserror**: Error handling

## Test Coverage

- 1 integration test file: `tests/integration_tests.rs`

## Quick Start

1. Build CLI: `cargo build --release`
2. Initialize: `myc init`
3. Run: `myc task create --title "Hello"`
4. GUI dev: `cd mycui && bun run tauri:dev`