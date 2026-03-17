# Agent Instructions

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

# List tasks and epics
myc list                    # Shows epics + tasks (tree view if dependencies exist)
myc list --epic 1
myc list --overdue
myc list --blocked
myc list --all              # Show all tasks including closed

# Manage dependencies (task 1 blocks task 2)
myc task link blocks --task 1 2
myc deps show 2

# Close tasks (blocked tasks cannot be closed without --force)
myc task close 1

# Batch operations (useful for bulk updates)
myc task batch-op close 1 2 3 [--force]     # Close multiple tasks
myc task batch-op tag urgent 1 2 3          # Tag multiple tasks
myc task batch-op move 1 4 5 6              # Move tasks to epic (0 = no epic)

# Task notes (useful for progress tracking)
myc task note 1 "Progress update..."        # Add a note to a task
myc task notes 1                            # View all notes for a task

# Task cloning (useful for similar tasks)
myc task clone 1 [--title "New Title"]      # Clone a task with all metadata

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
- **Task Note**: A comment or progress note added to a task

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
3. Create tasks for new work: `myc task create --title "..." --description "..." --epic N`
4. Add progress notes to tasks: `myc task note <id> "Progress update..."`
5. Clone similar tasks: `myc task clone <id> --title "New task"`
6. Batch close tasks when done: `myc task batch-op close <id> [<id>...]`
7. Mark tasks complete when done: `myc task close N`
8. Use `--format json` for machine-readable output: `myc list --format json`
