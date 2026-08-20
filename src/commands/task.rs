use crate::cli::OutputFormat;
use crate::commands::{
    aid, confirm, eid, ensure_initialized, rid, tid, ERROR_PREFIX, INFO_PREFIX, SUCCESS_PREFIX,
    WARNING_PREFIX,
};
use crate::error::Result;
use crate::models::external_ref::parse_github_ref;
use crate::models::{Epic, Priority, Status};
use chrono::{Duration, Local, NaiveDate};
use colored::Colorize;
use comfy_table::{ContentArrangement, Table};
use std::collections::{HashMap, HashSet};
use std::fs;

#[allow(clippy::too_many_arguments)]
pub fn create(
    title: &str,
    description: Option<&str>,
    epic_id: Option<i64>,
    parent_id: Option<i64>,
    priority: &str,
    assignee_id: Option<i64>,
    due: Option<&str>,
    tags: Option<&str>,
    notes: Option<&str>,
    user_info: Option<&str>,
    template: Option<&str>,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;

    // Apply template if specified
    let (final_priority, final_tags) = if let Some(template_name) = template {
        apply_template(template_name, priority, tags)?
    } else {
        (priority.to_string(), tags.map(|t| t.to_string()))
    };

    // Parse relative dates
    let due_date = if let Some(d) = due {
        Some(parse_relative_date(d)?)
    } else {
        None
    };

    let priority_enum: Priority = final_priority.parse()?;

    let mut task = db.create_task(
        title,
        description,
        epic_id,
        priority_enum,
        assignee_id,
        due_date,
        final_tags.as_deref(),
        notes,
        user_info,
    )?;

    // Attach to a parent if requested. Done as a follow-up call (rather than a
    // create_task arg) so the validated set_parent guard (existence, no self,
    // no cycle) is the single source of truth. A create with a bad --parent
    // leaves the task created but top-level, and surfaces the error.
    if let Some(pid) = parent_id {
        db.set_parent(task.id, Some(pid))?;
        // Re-read so JSON/quiet output reflects the parent_id.
        if let Some(updated) = db.get_task(task.id)? {
            task = updated;
        }
    }

    if quiet {
        println!("{}", task.id);
        return Ok(());
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&task)?),
        OutputFormat::Table => {
            println!(
                "{} Created task {}: {}",
                SUCCESS_PREFIX.green(),
                tid(task.id).cyan(),
                task.title.bold()
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn list(
    epic_id: Option<i64>,
    status: Option<&str>,
    priority: Option<&str>,
    assignee_id: Option<i64>,
    blocked: bool,
    overdue: bool,
    tag: Option<&str>,
    all: bool,
    tree: bool,
    parent_id: Option<i64>,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let db = ensure_initialized()?;

    // Default to 'open' status unless --all is specified or a specific status is given
    let status_enum: Option<Status> = if all {
        None
    } else {
        status
            .map(|s| s.parse())
            .transpose()?
            .or(Some(Status::Open))
    };

    let priority_enum: Option<Priority> = priority.map(|p| p.parse()).transpose()?;

    let tasks = db.list_tasks(
        epic_id,
        status_enum,
        priority_enum,
        assignee_id,
        blocked,
        overdue,
        tag,
        parent_id,
    )?;

    if quiet {
        for task in &tasks {
            println!("{}", task.id);
        }
        return Ok(());
    }

    match format {
        // JSON already carries parent_id on every task, so a consumer can build
        // the tree itself — --tree changes only the human table rendering.
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&tasks)?),
        OutputFormat::Table => {
            if tasks.is_empty() {
                let status_msg = if all {
                    "No tasks found."
                } else {
                    "No open tasks found. Use --all to see all tasks."
                };
                println!("{} {}", INFO_PREFIX.blue(), status_msg);
                return Ok(());
            }

            // Fetch all epics for lookup
            let epics = db.list_epics()?;
            let epic_map: HashMap<i64, Epic> = epics.into_iter().map(|e| (e.id, e)).collect();

            if tree {
                print_parent_tree(&tasks, &db)?;
                return Ok(());
            }

            // Fetch all dependencies for the tasks we're showing
            let task_ids: Vec<i64> = tasks.iter().map(|t| t.id).collect();
            let deps = db.get_dependencies_for_tasks(&task_ids)?;

            // Check if any tasks have dependencies
            let has_dependencies = deps
                .iter()
                .any(|(_, (blocked_by, blocks))| !blocked_by.is_empty() || !blocks.is_empty());

            if has_dependencies {
                // Tree view
                print_tree_view(&tasks, &epic_map, &deps, &db)?;
            } else {
                // Grouped list view
                print_grouped_view(&tasks, &epic_map)?;
            }
        }
    }
    Ok(())
}

/// Render matched tasks as a parent>child hierarchy.
///
/// Two of the peer's constraints shape this:
/// 1. Filter must not orphan a matched child: if a child matched but its parent
///    did not, the parent is still shown as CONTEXT (dimmed, marked "· context")
///    so the child renders nested rather than floating at the root.
/// 2. Counting must not mislead: a parent line shows "(+N subtasks)" from the
///    matched set, and context parents are not counted as matches themselves.
fn print_parent_tree(tasks: &[crate::models::Task], db: &crate::db::Database) -> Result<()> {
    use std::collections::{HashMap, HashSet};

    let matched: HashSet<i64> = tasks.iter().map(|t| t.id).collect();
    let by_id: HashMap<i64, &crate::models::Task> = tasks.iter().map(|t| (t.id, t)).collect();

    // Resolve context parents: a matched task whose parent is NOT in the set.
    // Fetch those parent rows so we can show them as context.
    let mut context_parents: HashMap<i64, crate::models::Task> = HashMap::new();
    for t in tasks {
        if let Some(pid) = t.parent_id {
            if !matched.contains(&pid) && !context_parents.contains_key(&pid) {
                if let Some(parent) = db.get_task(pid)? {
                    context_parents.insert(pid, parent);
                }
            }
        }
    }

    // Roots to print at the top level: matched tasks with no parent, matched
    // tasks whose parent is also unmatched-and-not-context (defensive), and the
    // context parents themselves.
    let mut children_of: HashMap<i64, Vec<i64>> = HashMap::new();
    for t in tasks {
        if let Some(pid) = t.parent_id {
            if matched.contains(&pid) || context_parents.contains_key(&pid) {
                children_of.entry(pid).or_default().push(t.id);
            }
        }
    }

    // Print helper (recursive over the matched/context maps).
    fn line(
        id: i64,
        depth: usize,
        is_context: bool,
        by_id: &HashMap<i64, &crate::models::Task>,
        context_parents: &HashMap<i64, crate::models::Task>,
        children_of: &HashMap<i64, Vec<i64>>,
    ) {
        let indent = "  ".repeat(depth);
        let child_count = children_of.get(&id).map(|v| v.len()).unwrap_or(0);
        let suffix = if child_count > 0 {
            format!(" (+{child_count} subtask(s))")
        } else {
            String::new()
        };
        let (status, priority, title) = if is_context {
            let t = &context_parents[&id];
            (t.status, t.priority, t.title.clone())
        } else {
            let t = by_id[&id];
            (t.status, t.priority, t.title.clone())
        };
        let marker = if is_context {
            " · context".dimmed()
        } else {
            "".normal()
        };
        println!(
            "{}{} {} {}: {}{}{}",
            indent,
            status.emoji(),
            priority.emoji(),
            tid(id).cyan(),
            title,
            suffix,
            marker
        );
        if let Some(kids) = children_of.get(&id) {
            for &kid in kids {
                line(kid, depth + 1, false, by_id, context_parents, children_of);
            }
        }
    }

    // Top-level order: context parents first (they head their subtrees), then
    // matched roots (no parent, or parent not in view at all).
    let mut printed: HashSet<i64> = HashSet::new();
    for (&cid, _) in context_parents.iter() {
        line(cid, 0, true, &by_id, &context_parents, &children_of);
        printed.insert(cid);
    }
    for t in tasks {
        let is_root = match t.parent_id {
            None => true,
            Some(pid) => !matched.contains(&pid) && !context_parents.contains_key(&pid),
        };
        if is_root && !printed.contains(&t.id) {
            line(t.id, 0, false, &by_id, &context_parents, &children_of);
            printed.insert(t.id);
        }
    }
    println!("\n{} matching task(s).", matched.len());
    Ok(())
}

/// Print tasks in a tree view when dependencies exist
fn print_tree_view(
    tasks: &[crate::models::Task],
    _epic_map: &HashMap<i64, Epic>,
    deps: &HashMap<i64, (Vec<i64>, Vec<i64>)>,
    _db: &crate::db::Database,
) -> Result<()> {
    use std::collections::HashSet;

    // Get task lookup
    let task_map: HashMap<i64, &crate::models::Task> = tasks.iter().map(|t| (t.id, t)).collect();

    // Find root tasks (those not blocked by other tasks in the filtered list)
    let task_ids: HashSet<i64> = tasks.iter().map(|t| t.id).collect();
    let mut roots: Vec<i64> = Vec::new();
    let mut visited: HashSet<i64> = HashSet::new();

    for task in tasks {
        if let Some((blocked_by, _)) = deps.get(&task.id) {
            // Task is a root if none of its blockers are in the filtered list
            let has_filtered_blocker = blocked_by.iter().any(|id| task_ids.contains(id));
            if !has_filtered_blocker {
                roots.push(task.id);
            }
        } else {
            roots.push(task.id);
        }
    }

    // If no roots found (e.g., circular dependency in filtered list), show all
    if roots.is_empty() {
        roots = tasks.iter().map(|t| t.id).collect();
    }

    // Print epics section first
    let mut epic_tasks: HashMap<Option<i64>, Vec<i64>> = HashMap::new();
    for task in tasks {
        epic_tasks.entry(task.epic_id).or_default().push(task.id);
    }

    println!("\n{}", "📋 Tasks (Tree View)".bold().underline());

    let mut total_shown = 0;

    // Sort roots by epic, then priority
    let mut sorted_roots = roots.clone();
    sorted_roots.sort_by_key(|id| {
        task_map
            .get(id)
            .map(|t| {
                let epic_sort = t.epic_id.unwrap_or(0);
                let priority_sort = match t.priority {
                    crate::models::Priority::Critical => 1,
                    crate::models::Priority::High => 2,
                    crate::models::Priority::Medium => 3,
                    crate::models::Priority::Low => 4,
                };
                (epic_sort, priority_sort)
            })
            .unwrap_or((0, 5))
    });

    // Print each root and its tree
    for (i, root_id) in sorted_roots.iter().enumerate() {
        let is_last = i == sorted_roots.len() - 1;
        print_task_tree(
            *root_id,
            &task_map,
            deps,
            &task_ids,
            "",
            is_last,
            &mut visited,
            &mut total_shown,
        )?;
    }

    println!("\n{} {} task(s) shown", INFO_PREFIX.blue(), total_shown);

    Ok(())
}

/// Recursively print task tree
fn print_task_tree(
    task_id: i64,
    task_map: &HashMap<i64, &crate::models::Task>,
    deps: &HashMap<i64, (Vec<i64>, Vec<i64>)>,
    filtered_ids: &HashSet<i64>,
    prefix: &str,
    is_last: bool,
    visited: &mut std::collections::HashSet<i64>,
    total_shown: &mut usize,
) -> Result<()> {
    if !filtered_ids.contains(&task_id) {
        return Ok(());
    }

    if visited.contains(&task_id) {
        // Circular reference - show reference only
        if let Some(task) = task_map.get(&task_id) {
            let connector = if is_last { "└── " } else { "├── " };
            println!(
                "{}{}{}: {} (circular)",
                prefix,
                connector,
                tid(task_id),
                task.title.dimmed()
            );
        }
        return Ok(());
    }

    visited.insert(task_id);
    *total_shown += 1;

    if let Some(task) = task_map.get(&task_id) {
        let connector = if prefix.is_empty() {
            ""
        } else if is_last {
            "└── "
        } else {
            "├── "
        };
        let status_icon = if task.status == Status::Open {
            "○".normal()
        } else {
            "✓".green()
        };
        let priority_str = format!("{}", task.priority.emoji());

        let epic_str = task
            .epic_id
            .map(|id| format!(" [{}]", eid(id)))
            .unwrap_or_default();

        let overdue_str = if task.is_overdue() {
            " [OVERDUE]".red().bold().to_string()
        } else {
            String::new()
        };

        let blocked_str = if let Some((blocked_by, _)) = deps.get(&task_id) {
            let open_blockers: Vec<_> = blocked_by
                .iter()
                .filter(|id| {
                    task_map
                        .get(*id)
                        .map(|t| t.status == Status::Open)
                        .unwrap_or(false)
                })
                .collect();
            if !open_blockers.is_empty() {
                format!(
                    " [blocked by {}]",
                    open_blockers
                        .iter()
                        .map(|id| tid(**id))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .yellow()
                .to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        println!(
            "{}{}{} {} {}{}{}{}",
            prefix,
            connector,
            status_icon,
            priority_str,
            task.title.bold(),
            epic_str.dimmed(),
            overdue_str,
            blocked_str
        );

        // Print children (tasks blocked by this one)
        if let Some((_, blocks)) = deps.get(&task_id) {
            let children: Vec<_> = blocks
                .iter()
                .filter(|id| filtered_ids.contains(id))
                .copied()
                .collect();

            let child_prefix = if prefix.is_empty() {
                if is_last {
                    "    ".to_string()
                } else {
                    "│   ".to_string()
                }
            } else {
                format!("{}{}", prefix, if is_last { "    " } else { "│   " })
            };

            for (i, child_id) in children.iter().enumerate() {
                let child_last = i == children.len() - 1;
                print_task_tree(
                    *child_id,
                    task_map,
                    deps,
                    filtered_ids,
                    &child_prefix,
                    child_last,
                    visited,
                    total_shown,
                )?;
            }
        }
    }

    Ok(())
}

/// Print tasks grouped by epic
fn print_grouped_view(tasks: &[crate::models::Task], epic_map: &HashMap<i64, Epic>) -> Result<()> {
    println!("\n{}", "📁 Epics".bold().underline());

    // Print all epics (including empty ones)
    let mut all_epics: Vec<&Epic> = epic_map.values().collect();
    all_epics.sort_by_key(|e| e.id);

    if all_epics.is_empty() {
        println!("  No epics yet. Create one with: myc epic create --title \"Epic Name\"");
    } else {
        let mut epic_table = Table::new();
        epic_table.set_content_arrangement(ContentArrangement::Dynamic);
        epic_table.set_header(vec!["ID", "Title", "Status", "Tasks"]);

        for epic in &all_epics {
            let task_count = tasks.iter().filter(|t| t.epic_id == Some(epic.id)).count();
            epic_table.add_row(vec![
                eid(epic.id).cyan().to_string(),
                epic.title.clone(),
                format!("{} {}", epic.status.emoji(), epic.status),
                format!("{} tasks", task_count),
            ]);
        }

        println!("{}", epic_table);
    }

    // Group tasks by epic
    let mut epic_tasks: HashMap<Option<i64>, Vec<&crate::models::Task>> = HashMap::new();
    for task in tasks {
        epic_tasks.entry(task.epic_id).or_default().push(task);
    }

    // Sort epics: epics with tasks first, then by epic id
    let mut epic_order: Vec<Option<i64>> = epic_tasks.keys().copied().collect();
    epic_order.sort_by_key(|e| {
        match e {
            Some(id) => (0, *id),
            None => (1, 0), // No epic goes last
        }
    });

    let mut total_shown = 0;

    if !tasks.is_empty() {
        println!("\n{}", "📋 Tasks".bold().underline());

        for epic_id in epic_order {
            let tasks_in_epic = epic_tasks.get(&epic_id).unwrap();

            // Print epic header
            match epic_id {
                Some(id) => {
                    if let Some(epic) = epic_map.get(&id) {
                        let status_icon = if epic.status == Status::Open {
                            "📂"
                        } else {
                            "📁"
                        };
                        println!(
                            "\n{} {} {} {}",
                            status_icon,
                            format!("{}:", eid(id)).cyan().bold(),
                            epic.title.bold(),
                            format!("({} tasks)", tasks_in_epic.len()).dimmed()
                        );
                    } else {
                        println!(
                            "\n{} {} {}",
                            "📂".cyan(),
                            format!("{}:", eid(id)).cyan().bold(),
                            format!("(Unknown epic, {} tasks)", tasks_in_epic.len()).dimmed()
                        );
                    }
                }
                None => {
                    println!(
                        "\n{} {}",
                        "📂 No Epic:".cyan().bold(),
                        format!("({} tasks)", tasks_in_epic.len()).dimmed()
                    );
                }
            }

            // Create table for tasks in this epic
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["ID", "Title", "Status", "Priority", "Due", "Tags"]);

            // Sort tasks by priority, then creation date
            let mut sorted_tasks = tasks_in_epic.clone();
            sorted_tasks.sort_by(|a, b| {
                let priority_ord = format!("{:?}", a.priority).cmp(&format!("{:?}", b.priority));
                if priority_ord != std::cmp::Ordering::Equal {
                    priority_ord
                } else {
                    b.created_at.cmp(&a.created_at)
                }
            });

            for task in sorted_tasks {
                let due = task
                    .due_date
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let due_str = if task.is_overdue() {
                    due.red().to_string()
                } else {
                    due
                };
                let tags = task
                    .tags
                    .as_ref()
                    .map(|t| {
                        // Truncate on char boundaries, not raw bytes: byte-slicing
                        // &t[..15] panics when a multi-byte UTF-8 char straddles
                        // offset 15 (accented tags, emoji). Tags are free-form input.
                        if t.chars().count() > 15 {
                            format!("{}...", t.chars().take(15).collect::<String>())
                        } else {
                            t.clone()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                table.add_row(vec![
                    tid(task.id).dimmed().to_string(),
                    task.title.clone(),
                    format!("{} {}", task.status.emoji(), task.status),
                    format!("{} {}", task.priority.emoji(), task.priority),
                    due_str,
                    tags,
                ]);
                total_shown += 1;
            }

            println!("{}", table);
        }
    }

    let status_filter_msg = if tasks.iter().all(|t| t.status == Status::Open) {
        " (open tasks only, use --all for all)"
    } else {
        ""
    };

    println!(
        "\n{} {} task(s) shown{}",
        INFO_PREFIX.blue(),
        total_shown,
        status_filter_msg.dimmed()
    );

    Ok(())
}

pub fn show(id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let task = db
        .get_task(id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "task".to_string(),
            id: id.to_string(),
        })?;

    let epic_title = task
        .epic_id
        .and_then(|eid| db.get_epic(eid).ok().flatten().map(|e| e.title));
    let assignee = task
        .assignee_id
        .and_then(|aid| db.get_assignee(aid).ok().flatten().map(|a| a.name));
    let blocking = db.get_blocking_tasks(id)?;
    let blocked_by = db.get_blocked_tasks(id)?;
    let refs = db.list_external_refs(id)?;

    if quiet {
        println!("{}", task.id);
        return Ok(());
    }

    match format {
        OutputFormat::Json => {
            let data = serde_json::json!({
                "task": task,
                "epic_title": epic_title,
                "assignee_name": assignee,
                "blocked_by": blocking,
                "blocks": blocked_by,
                "external_refs": refs,
            });
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        OutputFormat::Table => {
            println!("{} Task {}", INFO_PREFIX.blue(), tid(task.id).cyan().bold());
            println!("  Title: {}", task.title.bold());
            if let Some(desc) = &task.description {
                println!("  Description: {}", desc);
            }
            println!("  Status: {} {}", task.status.emoji(), task.status);
            println!("  Priority: {} {}", task.priority.emoji(), task.priority);
            if let Some(epic) = epic_title {
                println!("  Epic: {}", epic);
            }
            if let Some(assignee) = assignee {
                println!("  Assignee: {}", assignee);
            }
            if let Some(due) = task.due_date {
                if task.is_overdue() {
                    println!(
                        "  Due: {} {}",
                        due.to_string().red().bold(),
                        "OVERDUE".red().bold()
                    );
                } else {
                    println!("  Due: {}", due);
                }
            }
            if let Some(tags) = &task.tags {
                println!("  Tags: {}", tags.yellow());
            }
            if let Some(notes) = &task.notes {
                println!("  Notes: {}", notes);
            }
            if let Some(user_info) = &task.user_info {
                println!("  User Info: {}", user_info);
            }
            if let Some(agent_questions) = &task.agent_questions {
                println!("  Agent Questions: {}", agent_questions.cyan());
            }
            println!("  Created: {}", task.created_at.format("%Y-%m-%d %H:%M"));
            println!();

            if !blocking.is_empty() {
                println!(
                    "  Blocked by: {}",
                    blocking
                        .iter()
                        .map(|id| tid(*id))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            if !blocked_by.is_empty() {
                println!(
                    "  Blocks: {}",
                    blocked_by
                        .iter()
                        .map(|id| tid(*id))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
            if !refs.is_empty() {
                println!("  References:");
                for r in refs {
                    println!("    {} {}: {}", r.ref_type.emoji(), r.ref_type, r.reference);
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
    priority: Option<&str>,
    epic_id: Option<i64>,
    parent_id: Option<i64>,
    assignee_id: Option<i64>,
    due: Option<&str>,
    tags: Option<&str>,
    notes: Option<&str>,
    user_info: Option<&str>,
    agent_questions: Option<&str>,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;

    let status_enum = status.map(|s| s.parse::<Status>()).transpose()?;
    let priority_enum = priority.map(|p| p.parse::<Priority>()).transpose()?;
    let due_date = if let Some(d) = due {
        Some(Some(parse_relative_date(d)?))
    } else {
        None
    };

    let epic_opt = epic_id.map(|e| if e == 0 { None } else { Some(e) });
    let assignee_opt = assignee_id.map(|a| if a == 0 { None } else { Some(a) });
    let tags_opt = tags.map(|t| if t == "-" { None } else { Some(t) });
    let notes_opt = notes.map(|n| if n == "-" { None } else { Some(n) });
    let user_info_opt = user_info.map(|u| if u == "-" { None } else { Some(u) });
    let agent_questions_opt = agent_questions.map(|q| if q == "-" { None } else { Some(q) });

    let mut task = db.update_task(
        id,
        title,
        description,
        status_enum,
        priority_enum,
        epic_opt,
        assignee_opt,
        due_date,
        tags_opt,
        notes_opt,
        user_info_opt,
        agent_questions_opt,
    )?;

    // Parent re-assignment (0 = detach). Applied through set_parent so the
    // cycle/self/existence guard runs; only when --parent was actually passed.
    if let Some(p) = parent_id {
        let new_parent = if p == 0 { None } else { Some(p) };
        db.set_parent(id, new_parent)?;
        if let Some(updated) = db.get_task(id)? {
            task = updated;
        }
    }

    if quiet {
        println!("{}", task.id);
        return Ok(());
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&task)?),
        OutputFormat::Table => {
            println!(
                "{} Updated task {}: {}",
                SUCCESS_PREFIX.green(),
                tid(task.id).cyan(),
                task.title.bold()
            );
        }
    }
    Ok(())
}

pub fn delete(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let task = db
        .get_task(id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "task".to_string(),
            id: id.to_string(),
        })?;

    if !force {
        if !crate::commands::confirm(&format!("Delete task {}: '{}'", tid(id), task.title)) {
            println!("Cancelled.");
            return Ok(());
        }
    }

    db.delete_task(id)?;

    if !quiet {
        println!(
            "{} Deleted task {}: {}",
            SUCCESS_PREFIX.green(),
            tid(id).cyan(),
            task.title
        );
    }
    Ok(())
}

pub fn assign(task_id: i64, assignee_id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let assignee_opt = if assignee_id == 0 {
        None
    } else {
        Some(assignee_id)
    };
    let _task = db.update_task(
        task_id,
        None,
        None,
        None,
        None,
        None,
        Some(assignee_opt),
        None,
        None,
        None,
        None,
        None,
    )?;

    if !quiet {
        if assignee_id == 0 {
            println!(
                "{} Unassigned task {}",
                SUCCESS_PREFIX.green(),
                tid(task_id)
            );
        } else {
            println!(
                "{} Assigned task {} to assignee {}",
                SUCCESS_PREFIX.green(),
                tid(task_id),
                aid(assignee_id)
            );
        }
    }
    Ok(())
}

pub fn link_github_issue(task_id: i64, reference: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let (owner, repo, number) = parse_github_ref(reference)
        .ok_or_else(|| crate::error::MyceliumError::InvalidGitHubRef(reference.to_string()))?;

    let _ext_ref = db.add_external_ref(
        task_id,
        crate::models::ExternalRefType::GitHubIssue,
        &format!("{}/{}/{}", owner, repo, number),
    )?;

    if !quiet {
        println!(
            "{} Linked task {} to GitHub issue {}",
            SUCCESS_PREFIX.green(),
            tid(task_id),
            reference.cyan()
        );
    }
    Ok(())
}

pub fn link_github_pr(task_id: i64, reference: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let (owner, repo, number) = parse_github_ref(reference)
        .ok_or_else(|| crate::error::MyceliumError::InvalidGitHubRef(reference.to_string()))?;

    let _ext_ref = db.add_external_ref(
        task_id,
        crate::models::ExternalRefType::GitHubPr,
        &format!("{}/{}/{}", owner, repo, number),
    )?;

    if !quiet {
        println!(
            "{} Linked task {} to GitHub PR {}",
            SUCCESS_PREFIX.green(),
            tid(task_id),
            reference.cyan()
        );
    }
    Ok(())
}

pub fn link_url(task_id: i64, url: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let _ext_ref = db.add_external_ref(task_id, crate::models::ExternalRefType::Url, url)?;

    if !quiet {
        println!(
            "{} Linked task {} to URL {}",
            SUCCESS_PREFIX.green(),
            tid(task_id),
            url.cyan()
        );
    }
    Ok(())
}

pub fn link_blocks(task_id: i64, blocked_id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    db.add_dependency(blocked_id, task_id)?;

    if !quiet {
        println!(
            "{} Task {} now blocks task {}",
            SUCCESS_PREFIX.green(),
            tid(task_id),
            tid(blocked_id)
        );
    }
    Ok(())
}

pub fn unlink_ref(ref_id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    db.remove_external_ref(ref_id)?;

    if !quiet {
        println!(
            "{} Removed external reference {}",
            SUCCESS_PREFIX.green(),
            rid(ref_id)
        );
    }
    Ok(())
}

pub fn close(id: i64, force: bool, cascade: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let _task = db
        .get_task(id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "task".to_string(),
            id: id.to_string(),
        })?;

    // Checked here too (not just in core) to keep the friendly multi-line
    // listing and the exit-0 "refused" behaviour this command has always had.
    let blockers = db.get_open_blockers(id)?;
    if !blockers.is_empty() && !force {
        println!(
            "{} Cannot close task {} - blocked by:",
            ERROR_PREFIX.red(),
            tid(id)
        );
        for blocker in &blockers {
            println!("  - {}: {}", tid(blocker.id), blocker.title);
        }
        println!("\nUse --force to close anyway (or resolve the blockers first).");
        return Ok(());
    }

    // Subtask handling: closing a parent never auto-closes its children. If
    // there are open children, decide whether to cascade:
    //   --cascade         -> close them all, no prompt
    //   interactive       -> prompt once; yes cascades, no closes only parent
    //   quiet (scripted)  -> close only the parent (no prompt possible)
    let open_children = db.get_open_children(id)?;
    let mut close_children = cascade;
    if !open_children.is_empty() && !cascade {
        if quiet {
            // Non-interactive: leave children open (safe default), parent closes.
        } else {
            println!(
                "{} Task {} has {} open subtask(s):",
                INFO_PREFIX,
                tid(id),
                open_children.len()
            );
            for c in &open_children {
                println!("  - {}: {}", tid(c.id), c.title);
            }
            close_children = confirm("Also close these subtasks?");
        }
    }

    let updated = db.update_task_forced(
        id,
        None,
        None,
        Some(Status::Closed),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )?;

    let mut closed_children = 0usize;
    if close_children {
        for c in &open_children {
            // Force past each child's own blockers: the user opted into closing
            // the whole subtree. A child that fails to close is reported, not
            // fatal.
            match db.update_task_forced(
                c.id,
                None,
                None,
                Some(Status::Closed),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ) {
                Ok(_) => closed_children += 1,
                Err(e) => {
                    if !quiet {
                        println!(
                            "{} Could not close subtask {}: {}",
                            ERROR_PREFIX.red(),
                            tid(c.id),
                            e
                        );
                    }
                }
            }
        }
    }

    if !quiet {
        println!(
            "{} Closed task {}: {}",
            SUCCESS_PREFIX.green(),
            tid(id),
            updated.title
        );
        if closed_children > 0 {
            println!("  (also closed {closed_children} subtask(s))");
        } else if !open_children.is_empty() && !close_children {
            println!("  ({} subtask(s) left open)", open_children.len());
        }
        crate::commands::followup::print_close_hint();
    }
    Ok(())
}

/// Add a non-blocking reference (relates/duplicate) between two tasks. Marks
/// only — never closes or merges. Symmetric: the stored row is direction-free.
pub fn link_ref(task_id: i64, other_id: i64, ref_type: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    let rt: crate::models::TaskRefType = ref_type.parse()?;
    db.add_task_ref(task_id, other_id, rt)?;
    if !quiet {
        println!(
            "{} Marked task {} and task {} as {} {}",
            SUCCESS_PREFIX.green(),
            tid(task_id),
            tid(other_id),
            rt,
            rt.emoji()
        );
    }
    Ok(())
}

/// Remove a non-blocking reference between two tasks (either direction).
pub fn ref_unlink(task_id: i64, other_id: i64, ref_type: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    let rt: crate::models::TaskRefType = ref_type.parse()?;
    let n = db.remove_task_ref(task_id, other_id, rt)?;
    if !quiet {
        if n > 0 {
            println!(
                "{} Removed {} reference between task {} and task {}",
                SUCCESS_PREFIX.green(),
                rt,
                tid(task_id),
                tid(other_id)
            );
        } else {
            println!(
                "{} No {} reference between task {} and task {}",
                INFO_PREFIX,
                rt,
                tid(task_id),
                tid(other_id)
            );
        }
    }
    Ok(())
}

/// Show the non-blocking references (relates/duplicate) touching a task.
pub fn refs(id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    db.get_task(id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "task".to_string(),
            id: id.to_string(),
        })?;
    let refs = db.get_task_refs(id)?;

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&refs)?),
        OutputFormat::Table => {
            if quiet {
                for r in &refs {
                    println!("{}\t{}\t{}", r.ref_type, tid(r.other_id), r.title);
                }
            } else if refs.is_empty() {
                println!("{} Task {} has no references.", INFO_PREFIX, tid(id));
            } else {
                println!("References for task {}:", tid(id));
                for r in &refs {
                    println!(
                        "  {} {} {}: {}",
                        r.ref_type.emoji(),
                        r.ref_type,
                        tid(r.other_id).cyan(),
                        r.title
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn reopen(id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let updated = db.update_task(
        id,
        None,
        None,
        Some(Status::Open),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )?;

    if !quiet {
        println!(
            "{} Reopened task {}: {}",
            SUCCESS_PREFIX.green(),
            tid(id),
            updated.title
        );
    }
    Ok(())
}

/// Batch create tasks from a JSON file
pub fn batch(file_path: &str, format: &OutputFormat, quiet: bool) -> Result<()> {
    let content = fs::read_to_string(file_path)?;
    let tasks: Vec<BatchTaskInput> = serde_json::from_str(&content)?;

    let mut db = ensure_initialized()?;
    let mut created_ids = Vec::new();

    for task_input in tasks {
        let priority_enum: Priority = task_input.priority.parse()?;

        // Parse relative date if provided
        let due_date = if let Some(d) = &task_input.due {
            Some(parse_relative_date(d)?)
        } else {
            None
        };

        let task = db.create_task(
            &task_input.title,
            task_input.description.as_deref(),
            task_input.epic_id,
            priority_enum,
            task_input.assignee_id,
            due_date,
            task_input.tags.as_deref(),
            task_input.notes.as_deref(),
            task_input.user_info.as_deref(),
        )?;

        created_ids.push(task.id);

        // Handle dependencies if specified
        if let Some(blocked_by) = task_input.blocked_by {
            for blocker_id in blocked_by {
                db.add_dependency(task.id, blocker_id)?;
            }
        }

        // Handle external refs if specified
        if let Some(refs) = task_input.external_refs {
            for ext_ref in refs {
                match ext_ref.ref_type.as_str() {
                    "github-issue" => {
                        let (owner, repo, number) = parse_github_ref(&ext_ref.reference)
                            .ok_or_else(|| {
                                crate::error::MyceliumError::InvalidGitHubRef(
                                    ext_ref.reference.clone(),
                                )
                            })?;
                        db.add_external_ref(
                            task.id,
                            crate::models::ExternalRefType::GitHubIssue,
                            &format!("{}/{}/{}", owner, repo, number),
                        )?;
                    }
                    "github-pr" => {
                        let (owner, repo, number) = parse_github_ref(&ext_ref.reference)
                            .ok_or_else(|| {
                                crate::error::MyceliumError::InvalidGitHubRef(
                                    ext_ref.reference.clone(),
                                )
                            })?;
                        db.add_external_ref(
                            task.id,
                            crate::models::ExternalRefType::GitHubPr,
                            &format!("{}/{}/{}", owner, repo, number),
                        )?;
                    }
                    "url" => {
                        db.add_external_ref(
                            task.id,
                            crate::models::ExternalRefType::Url,
                            &ext_ref.reference,
                        )?;
                    }
                    _ => {}
                }
            }
        }
    }

    if !quiet {
        match format {
            OutputFormat::Json => {
                let result = serde_json::json!({
                    "created": created_ids.len(),
                    "task_ids": created_ids,
                });
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Table => {
                println!(
                    "{} Created {} task(s)",
                    SUCCESS_PREFIX.green(),
                    created_ids.len()
                );
                for id in &created_ids {
                    println!("  - Task {}", tid(*id));
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct BatchTaskInput {
    title: String,
    description: Option<String>,
    epic_id: Option<i64>,
    priority: String,
    assignee_id: Option<i64>,
    due: Option<String>,
    tags: Option<String>,
    notes: Option<String>,
    user_info: Option<String>,
    blocked_by: Option<Vec<i64>>,
    external_refs: Option<Vec<BatchExternalRef>>,
}

#[derive(Debug, serde::Deserialize)]
struct BatchExternalRef {
    ref_type: String,
    reference: String,
}

/// Parse relative dates like "tomorrow", "in 3 days", "next week"
fn parse_relative_date(input: &str) -> Result<NaiveDate> {
    let input = input.to_lowercase();
    let today = Local::now().naive_local().date();

    if input == "today" {
        return Ok(today);
    }

    if input == "tomorrow" || input == "tmrw" {
        return Ok(today + Duration::days(1));
    }

    if input == "next week" {
        return Ok(today + Duration::weeks(1));
    }

    // Parse "in X days/weeks"
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() >= 3 && parts[0] == "in" {
        if let Ok(n) = parts[1].parse::<i64>() {
            match parts[2] {
                "day" | "days" => return Ok(today + Duration::days(n)),
                "week" | "weeks" => return Ok(today + Duration::weeks(n)),
                _ => {}
            }
        }
    }

    // Try to parse as standard date
    NaiveDate::parse_from_str(&input, "%Y-%m-%d")
        .map_err(|_| crate::error::MyceliumError::InvalidDate(input.to_string()))
}

/// Apply a template to get default priority and tags
fn apply_template(
    template: &str,
    priority: &str,
    tags: Option<&str>,
) -> Result<(String, Option<String>)> {
    // Check for custom templates file
    let custom_templates_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("templates.toml");

    if custom_templates_path.exists() {
        if let Ok(content) = fs::read_to_string(&custom_templates_path) {
            if let Ok(toml) = content.parse::<toml::Value>() {
                if let Some(template_table) = toml.get(template) {
                    let tpl_priority = template_table
                        .get("priority")
                        .and_then(|v| v.as_str())
                        .unwrap_or(priority);

                    let tpl_tags = template_table.get("tags").and_then(|v| v.as_str());

                    let final_tags = if let Some(t) = tags {
                        Some(t.to_string())
                    } else {
                        tpl_tags.map(|t| t.to_string())
                    };

                    return Ok((tpl_priority.to_string(), final_tags));
                }
            }
        }
    }

    // Built-in templates
    match template {
        "bug" => Ok(("high".to_string(), Some("bug".to_string()))),
        "feature" => Ok(("medium".to_string(), Some("feature".to_string()))),
        "docs" => Ok(("low".to_string(), Some("documentation".to_string()))),
        "refactor" => Ok(("medium".to_string(), Some("refactor".to_string()))),
        "test" => Ok(("medium".to_string(), Some("testing".to_string()))),
        _ => {
            // Unknown template, just use provided values
            Ok((priority.to_string(), tags.map(|t| t.to_string())))
        }
    }
}

/// Add a note to a task
pub fn add_note(task_id: i64, content: &str, format: &OutputFormat, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    // Verify task exists
    let task = db
        .get_task(task_id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "task".to_string(),
            id: task_id.to_string(),
        })?;

    let note = db.add_task_note(task_id, content)?;

    if quiet {
        println!("{}", note.id);
        return Ok(());
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&note)?),
        OutputFormat::Table => {
            println!(
                "{} Added note #{} to task {}: {}",
                SUCCESS_PREFIX.green(),
                note.id.to_string().cyan(),
                tid(task_id).cyan(),
                task.title.bold()
            );
        }
    }
    Ok(())
}

/// Show notes for a task
pub fn show_notes(task_id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;

    // Verify task exists
    let task = db
        .get_task(task_id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "task".to_string(),
            id: task_id.to_string(),
        })?;

    let notes = db.list_task_notes(task_id)?;

    if quiet {
        for note in &notes {
            println!("{}", note.id);
        }
        return Ok(());
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&notes)?),
        OutputFormat::Table => {
            println!(
                "{} Notes for task {}: {}",
                INFO_PREFIX.blue(),
                tid(task_id).cyan(),
                task.title.bold()
            );

            if notes.is_empty() {
                println!("\n  No notes yet.");
            } else {
                println!();
                for note in notes {
                    println!(
                        "  {} {}: {}",
                        "📝".cyan(),
                        note.created_at
                            .format("%Y-%m-%d %H:%M")
                            .to_string()
                            .dimmed(),
                        note.content
                    );
                }
            }
        }
    }
    Ok(())
}

/// Clone a task
pub fn clone_task(
    task_id: i64,
    new_title: Option<&str>,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;

    let original = db
        .get_task(task_id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "task".to_string(),
            id: task_id.to_string(),
        })?;

    let new_task = db.clone_task(task_id, new_title)?;

    if quiet {
        println!("{}", new_task.id);
        return Ok(());
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&new_task)?),
        OutputFormat::Table => {
            println!(
                "{} Cloned task {} to {}: {}",
                SUCCESS_PREFIX.green(),
                tid(task_id).cyan(),
                tid(new_task.id).cyan().bold(),
                new_task.title.bold()
            );
            println!("  Original: {}", original.title.dimmed());
        }
    }
    Ok(())
}

/// Batch close multiple tasks
pub fn batch_close(task_ids: &[i64], force: bool, quiet: bool) -> Result<()> {
    if task_ids.is_empty() {
        return Ok(());
    }

    let mut db = ensure_initialized()?;
    let outcome = db.batch_close_tasks(task_ids, force)?;

    if !quiet {
        if outcome.closed.is_empty() {
            println!("{} No tasks closed.", INFO_PREFIX.blue());
        } else {
            println!(
                "{} Closed {} task(s)",
                SUCCESS_PREFIX.green(),
                outcome.closed.len()
            );
            for task in &outcome.closed {
                println!("  - {}: {}", tid(task.id), task.title);
            }
        }

        if !outcome.blocked.is_empty() {
            let ids = outcome
                .blocked
                .iter()
                .map(|id| tid(*id))
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{} Skipped {} blocked task(s) (use --force): {}",
                INFO_PREFIX.blue(),
                outcome.blocked.len(),
                ids
            );
        }

        if !outcome.already_closed.is_empty() {
            println!(
                "{} {} task(s) already closed",
                INFO_PREFIX.blue(),
                outcome.already_closed.len()
            );
        }

        if !outcome.not_found.is_empty() {
            let ids = outcome
                .not_found
                .iter()
                .map(|id| tid(*id))
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{} {} not found: {}",
                WARNING_PREFIX.yellow(),
                outcome.not_found.len(),
                ids
            );
        }

        if !outcome.closed.is_empty() {
            crate::commands::followup::print_close_hint();
        }
    }

    Ok(())
}

/// Batch tag multiple tasks
pub fn batch_tag(tag: &str, task_ids: &[i64], quiet: bool) -> Result<()> {
    if task_ids.is_empty() {
        return Ok(());
    }

    let mut db = ensure_initialized()?;
    let (updated_tasks, not_found) = db.batch_add_tag(task_ids, tag)?;

    if !quiet {
        println!(
            "{} Added tag '{}' to {} task(s)",
            SUCCESS_PREFIX.green(),
            tag.yellow(),
            updated_tasks.len()
        );
        for task in &updated_tasks {
            println!("  - {}: {}", tid(task.id), task.title);
        }
        if !not_found.is_empty() {
            let ids = not_found
                .iter()
                .map(|id| tid(*id))
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{} Skipped {} not found: {}",
                WARNING_PREFIX.yellow(),
                not_found.len(),
                ids
            );
        }
    }

    Ok(())
}

/// Batch move multiple tasks to an epic
pub fn batch_move(epic_id: i64, task_ids: &[i64], quiet: bool) -> Result<()> {
    if task_ids.is_empty() {
        return Ok(());
    }

    let mut db = ensure_initialized()?;

    // Convert 0 to None (no epic)
    let epic_id_opt = if epic_id == 0 { None } else { Some(epic_id) };

    // Get epic name for display
    let epic_name = if let Some(eid_val) = epic_id_opt {
        match db.get_epic(eid_val)? {
            Some(epic) => format!("{}: {}", eid(eid_val), epic.title),
            None => format!("{} (not found)", eid(eid_val)),
        }
    } else {
        "No Epic".to_string()
    };

    let (updated_tasks, not_found) = db.batch_move_to_epic(task_ids, epic_id_opt)?;

    if !quiet {
        println!(
            "{} Moved {} task(s) to {}",
            SUCCESS_PREFIX.green(),
            updated_tasks.len(),
            epic_name.cyan()
        );
        for task in &updated_tasks {
            println!("  - {}: {}", tid(task.id), task.title);
        }
        if !not_found.is_empty() {
            let ids = not_found
                .iter()
                .map(|id| tid(*id))
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "{} Skipped {} not found: {}",
                WARNING_PREFIX.yellow(),
                not_found.len(),
                ids
            );
        }
    }

    Ok(())
}

pub fn batch_delete_orphans(force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    let orphans = db.list_orphan_tasks()?;

    if orphans.is_empty() {
        if !quiet {
            println!("{} No tasks without an epic found.", INFO_PREFIX);
        }
        return Ok(());
    }

    if !quiet {
        println!(
            "{} Found {} task(s) not assigned to any epic:",
            WARNING_PREFIX.yellow(),
            orphans.len()
        );
        for task in &orphans {
            println!("  - {}: {} [{}]", tid(task.id), task.title, task.status);
        }
    }

    if !force {
        if !confirm(&format!("Delete all {} orphan task(s)?", orphans.len())) {
            if !quiet {
                println!("  Cancelled.");
            }
            return Ok(());
        }
    }

    let count = db.delete_orphan_tasks()?;
    if !quiet {
        println!(
            "{} Deleted {} task(s) without an epic.",
            SUCCESS_PREFIX.green(),
            count
        );
    }

    Ok(())
}
