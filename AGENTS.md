# Agent Instructions

## Project Management with Mycelium

This project uses [Mycelium](https://github.com/tcsenpai/mycelium) (`myc`) for task and epic management.

### Quick Reference

```bash
# Initialize mycelium in this project (creates .mycelium/ directory)
myc init

# Create an epic (a large body of work)
myc epic create --title "Feature X" --description "Build feature X"
myc epic create --title "Feature X" --notes "Context for agents" --user-info "Extra details"

# Create tasks within an epic
myc task create --title "Implement Y" --description "Build Y" --epic 1 --priority high --due 2025-12-31
myc task create --title "Fix Z" --notes "Check the auth module" --user-info "Related to ticket ABC-123"

# Task priorities: low, medium, high, critical
# Task status: open, closed

# List tasks and epics
myc list                    # Shows epics + tasks (tree view if dependencies exist)
myc list --epic 1
myc list --overdue
myc list --blocked
myc list --all              # Show all tasks including closed
myc list --tag "frontend"   # Filter by tag

# Manage dependencies (task 1 blocks task 2)
myc task link blocks --task 1 2
myc deps show 2

# Close tasks (blocked tasks cannot be closed without --force)
myc task close 1

# Batch operations (useful for bulk updates)
myc task batch-op close 1 2 3 [--force]     # Close multiple tasks
myc task batch-op tag urgent 1 2 3          # Tag multiple tasks
myc task batch-op move 1 4 5 6              # Move tasks to epic (0 = no epic)
myc task batch-op delete-orphans [--force]  # Delete tasks without an epic

# Task notes (append-only log entries for progress tracking)
myc task note 1 "Progress update..."        # Add a note to a task
myc task notes 1                            # View all notes for a task

# Epic notes (same as task notes, but for epics)
myc epic note 1 "Sprint planning notes..."  # Add a note to an epic
myc epic notes 1                            # View all notes for an epic

# Notes, user info, and agent questions (inline fields on tasks/epics)
myc task update 1 --notes "Implementation tip: use the adapter pattern"
myc task update 1 --user-info "This is a high-priority item for the Q2 release"
myc task update 1 --agent-questions "Should we use sync or async here?"
myc epic update 1 --notes "Overall design notes" --agent-questions "Need clarification on scope"

# Use - to clear a field:
myc task update 1 --notes -
myc epic update 1 --agent-questions -

# Task cloning (useful for similar tasks)
myc task clone 1 [--title "New Title"]      # Clone a task with all metadata

# Assign tasks
myc assignee create --name "Alice" --github "alice"
myc task assign 1 1

# Link to external resources
myc task link github-issue --task 1 "owner/repo#123"
myc task link github-pr --task 1 "owner/repo#456"
myc task link url --task 1 "https://example.com"

# Tags (comma-separated, stored on tasks)
myc task create --title "Fix login" --tags "frontend,urgent"
myc task update 1 --tags "frontend,urgent,auth"  # Replace tags
myc task update 1 --tags -                        # Remove all tags
myc task batch-op tag "backend" 1 2 3             # Add tag to multiple tasks
myc list --tag "frontend"                         # Filter by tag

# Project overview
myc summary

# Export data
myc export json
myc export csv

# Linear integration (bidirectional sync)
myc linear setup            # Configure API key, team, mappings
myc linear sync             # Bidirectional sync
myc linear push             # Push local to Linear
myc linear pull             # Pull Linear to local
myc linear status           # Show sync status
myc linear unlink           # Remove integration
```

### Data Model

- **Epic**: A large body of work with title, description, notes, user info, and agent questions
- **Task**: A unit of work with title, description, notes, user info, agent questions, tags, priority, due date, and optional epic/assignee links
- **Dependency**: Task A blocks Task B (B cannot close until A is closed)
- **Assignee**: Person assigned to a task (can have GitHub username)
- **External Ref**: Link to GitHub issues/PRs or URLs
- **Task Note**: An append-only comment/progress log entry on a task
- **Epic Note**: An append-only comment/progress log entry on an epic

### Field Guide for Agents

| Field | Where | Purpose |
|-------|-------|---------|
| `notes` | Task, Epic | Free-form text for comments, tips, or implementation context. Set via `--notes` on create/update. |
| `user_info` | Task, Epic | Additional context from the user for agents or collaborators. Set via `--user-info` on create/update. |
| `agent_questions` | Task, Epic | Questions from the agent that need user clarification. Set via `--agent-questions` on update. |
| `tags` | Task | Comma-separated labels for categorization and filtering. Set via `--tags` on create/update, or `batch-op tag`. |
| Task Notes | Task | Append-only log via `myc task note <id> "..."`. View with `myc task notes <id>`. |
| Epic Notes | Epic | Append-only log via `myc epic note <id> "..."`. View with `myc epic notes <id>`. |

### Tags

Tags are comma-separated strings stored on tasks. They are free-form (no predefined list).

- **Set tags on create**: `myc task create --title "..." --tags "frontend,urgent"`
- **Replace tags**: `myc task update 1 --tags "new-tag1,new-tag2"`
- **Remove all tags**: `myc task update 1 --tags -`
- **Add tag to multiple tasks**: `myc task batch-op tag "my-tag" 1 2 3`
- **Filter by tag**: `myc list --tag "frontend"` or `myc task list --tag "frontend"`

Tags are matched with substring search (e.g., `--tag "front"` matches `"frontend"`).

### Git Tracking

The `.mycelium/` directory contains the SQLite database and should be committed to git:

```bash
git add .mycelium/
git commit -m "Add mycelium project tracking"
```

### For AI Agents

When working on this project:

1. Check existing tasks: `myc list`
2. Check blocked tasks: `myc list --blocked`
3. Read task details and context: `myc task show <id>` (shows notes, user info, agent questions)
4. Read epic details and context: `myc epic show <id>`
5. Create tasks for new work: `myc task create --title "..." --description "..." --epic N`
6. Add progress notes to tasks: `myc task note <id> "Progress update..."`
7. Add progress notes to epics: `myc epic note <id> "Sprint update..."`
8. Set agent questions when you need clarification: `myc task update <id> --agent-questions "Should we...?"`
9. Clone similar tasks: `myc task clone <id> --title "New task"`
10. Batch close tasks when done: `myc task batch-op close <id> [<id>...]`
11. Mark tasks complete when done: `myc task close N`
12. Use `--format json` for machine-readable output: `myc list --format json`
