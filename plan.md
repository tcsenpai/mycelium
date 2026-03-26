# Linear Sync Feature — Implementation Plan

## Overview

Add bidirectional sync between mycelium and Linear. Config lives in `.mycelium/.linear/config.toml`. Sync state tracked in SQLite via `linear_sync` table. Linear API accessed via GraphQL over HTTP.

## Architecture

```
src/
  linear/
    mod.rs          — public API, re-exports
    config.rs       — config read/write/setup wizard
    client.rs       — GraphQL HTTP client (reqwest)
    sync.rs         — bidirectional sync engine
    mapping.rs      — status/priority/assignee/epic mapping logic
  commands/
    linear.rs       — CLI command handlers
```

## Config: `.mycelium/.linear/config.toml`

```toml
api_key = "lin_api_..."
team_id = "TEAM-UUID"
team_name = "My Team"
sync_enabled = true

[mapping]
epic_mode = "label"  # "label" (default) or "project"

[mapping.status]
open = "Todo"
closed = "Done"

[mapping.priority]
low = 4
medium = 3
high = 2
critical = 1
```

## DB Migration v4: `linear_sync` table

```sql
CREATE TABLE IF NOT EXISTS linear_sync (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    local_task_id INTEGER NOT NULL,
    linear_issue_id TEXT NOT NULL,
    linear_issue_identifier TEXT,  -- e.g. "TEAM-123"
    last_synced_at TEXT NOT NULL,
    last_local_hash TEXT NOT NULL,   -- hash of key fields to detect local changes
    last_remote_hash TEXT NOT NULL,  -- hash of key fields to detect remote changes
    sync_direction TEXT NOT NULL DEFAULT 'both', -- 'both', 'push', 'pull'
    FOREIGN KEY (local_task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    UNIQUE(local_task_id),
    UNIQUE(linear_issue_id)
);

CREATE INDEX IF NOT EXISTS idx_linear_sync_issue ON linear_sync(linear_issue_id);
```

## CLI: `myc linear <subcommand>`

| Subcommand | Description |
|---|---|
| `setup` | Interactive wizard: enter API key → auto-detect teams → pick team → configure epic mapping → save config |
| `sync` | Full bidirectional sync |
| `push` | Push local changes to Linear (create new issues, update changed ones) |
| `pull` | Pull Linear changes to local (create new tasks, update changed ones) |
| `status` | Show config, sync stats, last sync time |
| `unlink` | Remove Linear config and sync data |

## Sync Engine Logic

### Push (local → Linear)
1. Load all local tasks
2. For each task with a `linear_sync` entry: compute local hash, compare to `last_local_hash`. If different → update Linear issue via API
3. For each task WITHOUT a `linear_sync` entry: create Linear issue via API, store mapping
4. Update `linear_sync` entries with new hashes and timestamp

### Pull (Linear → local)
1. Fetch all issues from configured team via GraphQL
2. For each issue with a `linear_sync` entry: compute remote hash, compare to `last_remote_hash`. If different → update local task
3. For each issue WITHOUT a `linear_sync` entry: create local task, store mapping
4. Update `linear_sync` entries

### Bidirectional (sync)
1. Pull first (remote wins on conflicts by default)
2. Push second (local-only items get created remotely)
3. Conflict: if BOTH local and remote changed since last sync → warn user, remote wins by default
4. `--force-local` flag → local wins all conflicts
5. `--force-remote` flag → remote wins all conflicts

### Hash computation
Hash of: `title + description + status + priority + assignee + due_date + tags`
Used to detect whether local or remote changed since last sync.

## Mapping Logic

### Status
- local `open` → Linear workflow state name matching "Todo" or "In Progress" (configurable)
- local `closed` → Linear workflow state name matching "Done" or "Completed" (configurable)
- Linear states not matching → default to `open`

### Priority
- local `critical` → Linear `1` (Urgent)
- local `high` → Linear `2` (High)
- local `medium` → Linear `3` (Medium)
- local `low` → Linear `4` (Low)
- Linear `0` (No priority) → local `medium`

### Epics
- Mode `label`: epic title → Linear label. Create label if not exists.
- Mode `project`: epic title → Linear project. Create project if not exists.
- Configurable in `config.toml` under `[mapping] epic_mode`

### Assignees
- Match by: git email first, then git username, then local assignee email, then name
- `myc linear setup` shows available Linear team members and lets user map them
- Unmatched assignees: warn but don't fail

## Dependencies

Add to `Cargo.toml`:
```toml
reqwest = { version = "0.12", features = ["json", "blocking"] }
sha2 = "0.10"    # for hashing
```

Using `blocking` reqwest since mycelium is a synchronous CLI app (no async runtime).

## Error Handling

New error variants in `MyceliumError`:
```rust
#[error("Linear API error: {0}")]
LinearApi(String),

#[error("Linear config error: {0}")]
LinearConfig(String),

#[error("Linear sync conflict on task {task_id}: {message}")]
LinearSyncConflict { task_id: i64, message: String },
```

## Todo

### Phase 1: Foundation
- [x] Add `reqwest` and `sha2` dependencies to Cargo.toml
- [x] Add new error variants to `MyceliumError`
- [x] Create `src/linear/mod.rs` with module structure
- [x] Create `src/linear/config.rs` — config struct, read/write, `.linear/config.toml`
- [x] Add DB migration v4 with `linear_sync` table
- [x] Add `linear_sync` CRUD operations to `src/db/mod.rs`

### Phase 2: Linear API Client
- [x] Create `src/linear/client.rs` — GraphQL client with reqwest blocking
- [x] Implement: fetch teams, fetch team members, fetch workflow states
- [x] Implement: fetch issues (paginated), create issue, update issue
- [x] Implement: fetch/create labels, fetch/create projects

### Phase 3: Mapping & Sync
- [x] Create `src/linear/mapping.rs` — status, priority, assignee, epic mapping
- [x] Create `src/linear/sync.rs` — hash computation, push, pull, bidirectional sync
- [x] Implement conflict detection and resolution

### Phase 4: CLI Integration
- [x] Create `src/commands/linear.rs` — all subcommand handlers
- [x] Add `Linear` subcommand to `src/cli/mod.rs`
- [x] Wire up in `src/main.rs`
- [x] Add `pub mod linear;` to `src/commands/mod.rs` and `src/main.rs` modules

### Phase 5: Setup Wizard
- [x] Implement interactive setup: API key input → team auto-detect → team selection
- [x] Add assignee mapping step (show Linear members, match to local assignees)
- [x] Add epic mapping mode selection (label/project)
- [x] Save config to `.mycelium/.linear/config.toml`

### Phase 6: Testing & Polish
- [x] Compilation verified — builds cleanly
- [x] CLI help and subcommands working
- [x] Graceful errors when not configured
- [x] Add `.linear/` to `.mycelium/.gitignore` (handled in config.save())
