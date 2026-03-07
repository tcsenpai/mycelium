# Requirements: Mycelium v1

---

## v1 Requirements

### Epic Management (EPIC)

- [ ] **EPIC-01**: User can create an epic with title and description
- [ ] **EPIC-02**: User can update epic details (title, description, status)
- [ ] **EPIC-03**: User can delete an epic (with cascade or blocking based on tasks)
- [ ] **EPIC-04**: User can list all epics with summary counts (total/open/closed tasks)
- [ ] **EPIC-05**: User can view epic details with all associated tasks

### Task Management (TASK)

- [ ] **TASK-01**: User can create a task with title and description
- [ ] **TASK-02**: User can assign a task to an epic
- [ ] **TASK-03**: User can set task priority (Low/Medium/High/Critical)
- [ ] **TASK-04**: User can set task due date
- [ ] **TASK-05**: User can update task details (title, description, priority, due date)
- [ ] **TASK-06**: User can change task status (open/closed)
- [ ] **TASK-07**: User can delete a task
- [ ] **TASK-08**: User can list tasks with filtering (by epic, status, priority, assignee)
- [ ] **TASK-09**: User can view task details with all relationships

### Dependencies (DEPS)

- [ ] **DEPS-01**: User can mark a task as blocking another task
- [ ] **DEPS-02**: User can view all tasks blocked by a given task
- [ ] **DEPS-03**: User can view all tasks a given task is blocked by
- [ ] **DEPS-04**: System prevents closing a task that has open blockers
- [ ] **DEPS-05**: User can remove a dependency relationship
- [ ] **DEPS-06**: System detects and prevents circular dependencies
- [ ] **DEPS-07**: User can visualize dependency chain for a task (tree/graph view)

### Assignees (ASGN)

- [ ] **ASGN-01**: User can assign a task to a local assignee (name/email)
- [ ] **ASGN-02**: User can link a local assignee to a GitHub username
- [ ] **ASGN-03**: User can filter tasks by assignee
- [ ] **ASGN-04**: User can list all assignees with their task counts

### External References (REF)

- [ ] **REF-01**: User can link a task to a GitHub issue (owner/repo#number)
- [ ] **REF-02**: User can link a task to a GitHub PR (owner/repo#number)
- [ ] **REF-03**: User can link a task to an arbitrary URL
- [ ] **REF-04**: User can view all external references on a task
- [ ] **REF-05**: User can remove an external reference

### Reporting & Queries (RPT)

- [ ] **RPT-01**: User can list blocked tasks (tasks with unresolved dependencies)
- [ ] **RPT-02**: User can list overdue tasks (past due date, not closed)
- [ ] **RPT-03**: User can list tasks by priority across all epics
- [ ] **RPT-04**: User can show project summary (epics, tasks, completion rates)
- [ ] **RPT-05**: User can export task list to JSON/CSV

### Storage & Data Integrity (DATA)

- [ ] **DATA-01**: All data stored in SQLite database in `.mycelium/mycelium.db`
- [ ] **DATA-02**: Database schema includes migrations system
- [ ] **DATA-03**: Database uses WAL mode for performance
- [ ] **DATA-04**: Data operations are atomic (transactions)
- [ ] **DATA-05**: Database format is git-friendly (no binary diffs on text changes)
- [ ] **DATA-06**: On init, tool creates `.mycelium/` directory with `.gitignore` entries

### CLI Interface (CLI)

- [ ] **CLI-01**: Binary responds to `myc` command
- [ ] **CLI-02**: Commands follow pattern: `myc <noun> <verb> [flags]`
- [ ] **CLI-03**: Help available via `--help` for all commands
- [ ] **CLI-04**: Clear error messages with actionable guidance
- [ ] **CLI-05**: Consistent exit codes (0=success, 1=error, 2=usage error)
- [ ] **CLI-06**: JSON output mode for all list/show commands (`--json`)
- [ ] **CLI-07**: Quiet mode for scripts (`--quiet`)

### Error Handling & Safety (SAFE)

- [ ] **SAFE-01**: All operations validate inputs before execution
- [ ] **SAFE-02**: Destructive operations require confirmation or `--force` flag
- [ ] **SAFE-03**: Database corruption detection on startup
- [ ] **SAFE-04**: Graceful handling of missing `.mycelium/` directory
- [ ] **SAFE-05**: Clear error when attempting operations on non-existent entities
- [ ] **SAFE-06**: Operations fail fast with descriptive messages

---

## v2 Requirements (Deferred)

- Task templates for common patterns
- Recurring tasks
- Task notes/comments with timestamps
- Advanced filtering (compound conditions)
- Task search (full-text)
- Custom fields on tasks
- Team velocity tracking
- Sprint/milestone planning beyond epics

---

## Out of Scope

| Feature | Reason |
|---------|--------|
| Time tracking (timers) | Not core to task management |
| Real-time sync | Git-based workflow is the target |
| Desktop notifications | CLI-focused, no GUI |
| Two-way GitHub sync | Adds complexity, one-way sufficient |
| Web UI | CLI-only by design |
| Multi-project workspace | Per-project isolation by design |
| Task attachments | Out of scope for v1 |
| Email notifications | CLI-focused tool |
| REST API | Local CLI tool only |

---

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| (To be filled by roadmap) | | |

---

*Last updated: 2026-03-07*
