# Mycelium

A robust, production-grade task/plan manager CLI designed for reliability and agent usage.

---

## Core Value

Provide a single-binary, fail-proof task management system that tracks work across epics, dependencies, priorities, and blockers while maintaining full git-trackable state for team collaboration.

---

## What We're Building

`myc` (mycelium) is a CLI tool that enables:

- **Epic-based organization** — Group related tasks into larger initiatives
- **Rich dependency tracking** — Task-to-task, external (GitHub), and visual dependency graphs
- **Flexible assignee management** — Local assignees + GitHub user sync
- **Git-trackable storage** — SQLite database designed for git diffs and collaboration
- **One-shot CLI design** — Optimized for agent/automation usage with traditional subcommands
- **Priority & blocker management** — Simple priority levels with clear blocking relationships
- **GitHub integration** — One-way linking to issues and PRs for context
- **Project-local data** — All data stored in `.mycelium/` directory

---

## Target Users

- Software development teams using git-based workflows
- AI agents and automation tools requiring structured task management
- Individual developers wanting lightweight, powerful task tracking
- Project managers needing dependency visualization and progress tracking

---

## Key Constraints

| Constraint | Value |
|------------|-------|
| Binary | Single static binary (`myc`) |
| Storage | SQLite (git-trackable) |
| CLI Style | Traditional subcommands, one-shot execution |
| Data Location | Project-local (`.mycelium/` directory) |
| GitHub Integration | One-way (reference only, no sync back) |
| Priority System | Simple (Low/Medium/High/Critical) |
| Sync | Pure git-based (no external servers) |

---

## Success Criteria

- [ ] Binary compiles to single static executable
- [ ] All data stored in git-trackable SQLite format
- [ ] Sub-100ms response time for typical operations
- [ ] Comprehensive error handling with clear messages
- [ ] Full test coverage for core operations
- [ ] Works reliably in CI/automation environments

---

## Non-Goals (Out of Scope)

| Feature | Reason |
|---------|--------|
| Time tracking | Adds complexity, not core to task management |
| Real-time sync | Git-based collaboration is the target |
| Desktop notifications | CLI-focused tool |
| Two-way GitHub sync | Adds complexity, one-way linking sufficient |
| Web UI | CLI-only by design |
| Cloud hosting | Self-hosted, git-based only |

---

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] User can create and manage epics as task containers
- [ ] User can create tasks with title, description, priority, and due dates
- [ ] User can assign tasks to local users or GitHub accounts
- [ ] User can establish task-to-task dependencies (blocks/blocked-by)
- [ ] User can link tasks to external resources (GitHub issues/PRs, URLs)
- [ ] User can visualize dependency chains and blocked tasks
- [ ] User can filter and list tasks by epic, assignee, priority, status
- [ ] User can mark tasks as complete with automatic dependency checking
- [ ] Data persists in SQLite format optimized for git diffs
- [ ] All operations are atomic and recoverable
- [ ] CLI provides clear error messages and exit codes
- [ ] Commands complete in under 100ms for typical operations

### Out of Scope

- Time tracking — Not required for core functionality
- Real-time notifications — Git-based workflow sufficient
- Two-way GitHub sync — One-way linking reduces complexity
- Web interface — CLI-only by design
- Multi-project aggregation — Per-project isolation by design

---

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| SQLite over JSON/YAML | Performance + ACID + git-trackable with proper config | — Pending |
| One-shot CLI over TUI | Optimized for agent/automation usage | — Pending |
| Rust as implementation | Safety, performance, single binary output | — Pending |
| Project-local data store | Git-trackable, team collaboration | — Pending |
| One-way GitHub integration | Reduces complexity, sufficient for use case | — Pending |

---

## Notes

### Data Model Overview

```
Epic
├── id, title, description, status
└── Tasks[]

Task
├── id, title, description, status, priority
├── epic_id (optional)
├── assignee (local or GitHub)
├── due_date (optional)
├── dependencies (task_ids[])
├── blocked_by (task_ids[])
├── external_refs (GitHub issues/PRs, URLs)
└── created_at, updated_at
```

### CLI Structure

```
myc epic create --title "Feature X"
myc task create --title "Implement Y" --epic 1 --priority high
myc task link --task 5 --blocks 6
myc task link --task 5 --github-issue owner/repo#123
myc task list --epic 1 --status open
myc deps show --task 5
```

### Git-Trackable SQLite

- Use WAL mode with periodic checkpoints
- Store in `.mycelium/mycelium.db`
- Consider sqlite-diffable or similar for readable diffs
- Document merge conflict resolution workflow

---

*Last updated: 2026-03-07 after initialization*
