# Roadmap: Mycelium v1

---

## Overview

| Property | Value |
|----------|-------|
| **Total Phases** | 5 |
| **v1 Requirements** | 38 |
| **Target** | Production-ready CLI tool |

---

## Phase Summary

| # | Phase | Goal | Requirements | Success Criteria |
|---|-------|------|--------------|------------------|
| 1 | Foundation | Rust project setup, CLI framework, SQLite schema | 6 | 4 |
| 2 | Core Entities | Epic and Task CRUD operations | 11 | 4 |
| 3 | Dependencies | Task blocking, dependency chains, cycle detection | 7 | 4 |
| 4 | Integration | Assignees, GitHub linking, external references | 8 | 4 |
| 5 | Safety & Polish | Error handling, reporting, validation, tests | 6 | 4 |

---

## Phase Details

### Phase 1: Foundation

**Goal:** Establish the Rust project with CLI framework, SQLite integration, and database schema.

**Requirements:** DATA-01, DATA-02, DATA-03, DATA-06, CLI-01, SAFE-04

**Success Criteria:**
1. `cargo build` produces a working binary named `myc`
2. `myc init` creates `.mycelium/` directory with SQLite database
3. Database schema migrations run automatically on startup
4. CLI framework handles subcommands and help text

**Key Deliverables:**
- Rust project structure with Cargo.toml
- SQLite integration with `rusqlite` or `sqlx`
- Database schema for epics, tasks, assignees, dependencies, references
- Migration system for schema versioning
- `myc init` command
- Basic `myc --help` and `myc --version`

---

### Phase 2: Core Entities

**Goal:** Implement Epic and Task CRUD with filtering and listing.

**Requirements:** EPIC-01 through EPIC-05, TASK-01 through TASK-09, CLI-02, CLI-03

**Success Criteria:**
1. User can create, read, update, delete epics
2. User can create, read, update, delete tasks
3. Tasks can be assigned to epics
4. List commands support filtering by status, priority, epic
5. JSON output mode works for all list/show commands

**Key Deliverables:**
- `myc epic create/list/show/update/delete` commands
- `myc task create/list/show/update/delete` commands
- Priority enum (Low/Medium/High/Critical)
- Status enum (Open/Closed)
- Filtering logic for list commands
- `--json` flag for programmatic output

---

### Phase 3: Dependencies

**Goal:** Implement task dependencies with blocking, cycle detection, and visualization.

**Requirements:** DEPS-01 through DEPS-07, SAFE-02

**Success Criteria:**
1. User can mark task A as blocking task B
2. Closing a blocked task is prevented (with clear message)
3. Circular dependencies are detected and rejected
4. User can view dependency tree for any task
5. List of blocked tasks is queryable

**Key Deliverables:**
- `myc task link --blocks` and `myc task unlink` commands
- `myc deps show` command with tree visualization
- `myc task list --blocked` filter
- Dependency graph validation (cycle detection)
- Blocker check on task close (with `--force` override)

---

### Phase 4: Integration

**Goal:** Add assignee management and external references (GitHub, URLs).

**Requirements:** ASGN-01 through ASGN-04, REF-01 through REF-05, RPT-01, RPT-02

**Success Criteria:**
1. Tasks can be assigned to local assignees
2. Assignees can be linked to GitHub usernames
3. Tasks can reference GitHub issues and PRs
4. Overdue tasks list works correctly
5. Tasks can be filtered by assignee

**Key Deliverables:**
- `myc assignee create/list` commands
- `myc task assign` command
- `myc task link --github-issue` and `--github-pr` commands
- `myc task link --url` command
- `myc task list --overdue` filter
- `myc task list --assignee` filter

---

### Phase 5: Safety & Polish

**Goal:** Comprehensive error handling, reporting, validation, and test coverage.

**Requirements:** SAFE-01, SAFE-03, SAFE-05, SAFE-06, RPT-03, RPT-04, RPT-05, CLI-04, CLI-05, CLI-07, DATA-04, DATA-05

**Success Criteria:**
1. All inputs validated with clear error messages
2. Database transactions ensure atomicity
3. Project summary command shows useful statistics
4. Export to JSON/CSV works
5. Test coverage > 80% for core logic
6. Binary size optimized for distribution

**Key Deliverables:**
- Input validation layer
- Comprehensive error types and messages
- `myc summary` command
- `myc export --json` and `myc export --csv` commands
- Unit tests for domain logic
- Integration tests for CLI commands
- `.mycelium/.gitignore` setup
- README with installation and usage

---

## Requirement Mapping

| Requirement | Phase | Implemented |
|-------------|-------|-------------|
| EPIC-01 | 2 | ⬜ |
| EPIC-02 | 2 | ⬜ |
| EPIC-03 | 2 | ⬜ |
| EPIC-04 | 2 | ⬜ |
| EPIC-05 | 2 | ⬜ |
| TASK-01 | 2 | ⬜ |
| TASK-02 | 2 | ⬜ |
| TASK-03 | 2 | ⬜ |
| TASK-04 | 2 | ⬜ |
| TASK-05 | 2 | ⬜ |
| TASK-06 | 2 | ⬜ |
| TASK-07 | 2 | ⬜ |
| TASK-08 | 2 | ⬜ |
| TASK-09 | 2 | ⬜ |
| DEPS-01 | 3 | ⬜ |
| DEPS-02 | 3 | ⬜ |
| DEPS-03 | 3 | ⬜ |
| DEPS-04 | 3 | ⬜ |
| DEPS-05 | 3 | ⬜ |
| DEPS-06 | 3 | ⬜ |
| DEPS-07 | 3 | ⬜ |
| ASGN-01 | 4 | ⬜ |
| ASGN-02 | 4 | ⬜ |
| ASGN-03 | 4 | ⬜ |
| ASGN-04 | 4 | ⬜ |
| REF-01 | 4 | ⬜ |
| REF-02 | 4 | ⬜ |
| REF-03 | 4 | ⬜ |
| REF-04 | 4 | ⬜ |
| REF-05 | 4 | ⬜ |
| RPT-01 | 4 | ⬜ |
| RPT-02 | 4 | ⬜ |
| RPT-03 | 5 | ⬜ |
| RPT-04 | 5 | ⬜ |
| RPT-05 | 5 | ⬜ |
| DATA-01 | 1 | ⬜ |
| DATA-02 | 1 | ⬜ |
| DATA-03 | 1 | ⬜ |
| DATA-04 | 5 | ⬜ |
| DATA-05 | 5 | ⬜ |
| DATA-06 | 1 | ⬜ |
| CLI-01 | 1 | ⬜ |
| CLI-02 | 2 | ⬜ |
| CLI-03 | 2 | ⬜ |
| CLI-04 | 5 | ⬜ |
| CLI-05 | 5 | ⬜ |
| CLI-06 | 2 | ⬜ |
| CLI-07 | 5 | ⬜ |
| SAFE-01 | 5 | ⬜ |
| SAFE-02 | 3 | ⬜ |
| SAFE-03 | 5 | ⬜ |
| SAFE-04 | 1 | ⬜ |
| SAFE-05 | 5 | ⬜ |
| SAFE-06 | 5 | ⬜ |

---

*Last updated: 2026-03-07*
