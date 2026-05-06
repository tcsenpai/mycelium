use colored::Colorize;

use crate::db::Database;
use crate::error::Result;
use crate::models::Status;

use super::client::{LinearClient, LinearIssue, LinearLabel};
use super::config::LinearConfig;
use super::mapping;

#[derive(Debug, Clone)]
pub struct SyncStats {
    pub pushed_new: usize,
    pub pushed_updated: usize,
    pub pulled_new: usize,
    pub pulled_updated: usize,
    pub conflicts: usize,
    pub skipped_inactive: usize,
    pub errors: Vec<String>,
}

impl SyncStats {
    fn new() -> Self {
        Self {
            pushed_new: 0,
            pushed_updated: 0,
            pulled_new: 0,
            pulled_updated: 0,
            conflicts: 0,
            skipped_inactive: 0,
            errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConflictResolution {
    RemoteWins,
    LocalWins,
}

/// Resolve filter label IDs from names, creating any that don't exist.
fn resolve_filter_label_ids(
    client: &LinearClient,
    config: &LinearConfig,
    filter_labels: &[String],
    existing_labels: &[LinearLabel],
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for name in filter_labels {
        if let Some(label) = existing_labels.iter().find(|l| l.name.eq_ignore_ascii_case(name)) {
            ids.push(label.id.clone());
        } else {
            // Create the label if it doesn't exist
            let label = client.create_label(&config.team_id, name)?;
            ids.push(label.id.clone());
        }
    }
    Ok(ids)
}

pub fn push(
    db: &mut Database,
    client: &LinearClient,
    config: &LinearConfig,
    quiet: bool,
) -> Result<SyncStats> {
    let mut stats = SyncStats::new();
    let filter_labels = config.resolve_filter_labels();
    let states = client.fetch_workflow_states(&config.team_id)?;
    let members = client.fetch_team_members(&config.team_id)?;
    let existing_labels = client.fetch_labels(&config.team_id)?;
    let filter_label_ids = resolve_filter_label_ids(client, config, &filter_labels, &existing_labels)?;

    let tasks = db.list_all_tasks()?;

    for task in &tasks {
        // Skip closed local tasks on push
        if task.status == Status::Closed {
            continue;
        }

        let sync_entry = db.get_linear_sync_by_task(task.id)?;

        match sync_entry {
            Some(entry) => {
                let current_hash = mapping::hash_task(task);
                if current_hash != entry.last_local_hash {
                    let state_id = mapping::status_to_linear_state_id(&task.status, &states, config)?;
                    let priority = mapping::priority_to_linear(&task.priority, config);
                    let assignee_linear_id = resolve_assignee(db, task.assignee_id, config, &members)?;
                    let mut label_ids = get_label_ids_for_task(db, task, client, config, &existing_labels)?;
                    // Ensure filter labels are always present
                    for fid in &filter_label_ids {
                        if !label_ids.contains(fid) {
                            label_ids.push(fid.clone());
                        }
                    }

                    match client.update_issue(
                        &entry.linear_issue_id,
                        Some(&task.title),
                        task.description.as_deref(),
                        Some(priority),
                        Some(&state_id),
                        assignee_linear_id.as_deref(),
                        task.due_date.map(|d| d.to_string()).as_deref(),
                        if label_ids.is_empty() { None } else { Some(&label_ids) },
                    ) {
                        Ok(issue) => {
                            let remote_hash = mapping::hash_issue(&issue);
                            db.update_linear_sync(task.id, &current_hash, &remote_hash)?;
                            stats.pushed_updated += 1;
                            if !quiet {
                                println!("  {} Updated {} -> {}", "->".blue(), task.title, issue.identifier);
                            }
                        }
                        Err(e) => {
                            stats.errors.push(format!("Failed to update task {}: {}", task.id, e));
                        }
                    }
                }
            }
            None => {
                let state_id = mapping::status_to_linear_state_id(&task.status, &states, config)?;
                let priority = mapping::priority_to_linear(&task.priority, config);
                let assignee_linear_id = resolve_assignee(db, task.assignee_id, config, &members)?;
                let mut label_ids = get_label_ids_for_task(db, task, client, config, &existing_labels)?;
                // Ensure filter labels are always present on new issues
                for fid in &filter_label_ids {
                    if !label_ids.contains(fid) {
                        label_ids.push(fid.clone());
                    }
                }

                match client.create_issue(
                    &config.team_id,
                    &task.title,
                    task.description.as_deref(),
                    priority,
                    &state_id,
                    assignee_linear_id.as_deref(),
                    task.due_date.map(|d| d.to_string()).as_deref(),
                    &label_ids,
                ) {
                    Ok(issue) => {
                        let local_hash = mapping::hash_task(task);
                        let remote_hash = mapping::hash_issue(&issue);
                        db.create_linear_sync(task.id, &issue.id, Some(&issue.identifier), &local_hash, &remote_hash)?;
                        stats.pushed_new += 1;
                        if !quiet {
                            println!("  {} Created {} -> {}", "++".green(), task.title, issue.identifier);
                        }
                    }
                    Err(e) => {
                        stats.errors.push(format!("Failed to create issue for task {}: {}", task.id, e));
                    }
                }
            }
        }
    }

    Ok(stats)
}

pub fn pull(
    db: &mut Database,
    client: &LinearClient,
    config: &LinearConfig,
    quiet: bool,
) -> Result<SyncStats> {
    let mut stats = SyncStats::new();
    let filter_labels = config.resolve_filter_labels();

    if !quiet {
        if filter_labels.is_empty() {
            println!("  Fetching active issues from Linear...");
        } else {
            println!("  Fetching active issues with labels {:?}...", filter_labels);
        }
    }
    let issues = client.fetch_all_issues_filtered(&config.team_id, &filter_labels, true)?;

    if !quiet {
        println!("  Found {} matching issues", issues.len());
    }

    for issue in &issues {
        let sync_entry = db.get_linear_sync_by_issue(&issue.id)?;

        match sync_entry {
            Some(entry) => {
                let current_hash = mapping::hash_issue(issue);
                if current_hash != entry.last_remote_hash {
                    apply_remote_to_local(db, &entry, issue, &current_hash, quiet)?;
                    stats.pulled_updated += 1;
                }
            }
            None => {
                create_local_from_remote(db, issue, quiet)?;
                stats.pulled_new += 1;
            }
        }
    }

    Ok(stats)
}

pub fn sync(
    db: &mut Database,
    client: &LinearClient,
    config: &LinearConfig,
    conflict_resolution: ConflictResolution,
    quiet: bool,
) -> Result<SyncStats> {
    let mut stats = SyncStats::new();
    let filter_labels = config.resolve_filter_labels();
    let states = client.fetch_workflow_states(&config.team_id)?;
    let members = client.fetch_team_members(&config.team_id)?;
    let existing_labels = client.fetch_labels(&config.team_id)?;
    let filter_label_ids = resolve_filter_label_ids(client, config, &filter_labels, &existing_labels)?;

    if !quiet {
        if filter_labels.is_empty() {
            println!("  Fetching active issues from Linear...");
        } else {
            println!("  Fetching active issues with labels {:?}...", filter_labels);
        }
    }
    let issues = client.fetch_all_issues_filtered(&config.team_id, &filter_labels, true)?;

    if !quiet {
        println!("  Found {} matching issues", issues.len());
    }

    // Phase 1: Process remote issues (pull side)
    for issue in &issues {
        let sync_entry = db.get_linear_sync_by_issue(&issue.id)?;

        match sync_entry {
            Some(entry) => {
                let remote_hash = mapping::hash_issue(issue);
                let local_task = db.get_task(entry.local_task_id)?;

                if let Some(ref task) = local_task {
                    let local_hash = mapping::hash_task(task);
                    let remote_changed = remote_hash != entry.last_remote_hash;
                    let local_changed = local_hash != entry.last_local_hash;

                    if remote_changed && local_changed {
                        stats.conflicts += 1;
                        match conflict_resolution {
                            ConflictResolution::RemoteWins => {
                                apply_remote_to_local(db, &entry, issue, &remote_hash, quiet)?;
                                stats.pulled_updated += 1;
                            }
                            ConflictResolution::LocalWins => {
                                // Will be handled in push phase
                            }
                        }
                        if !quiet {
                            let winner = match conflict_resolution {
                                ConflictResolution::RemoteWins => "remote wins",
                                ConflictResolution::LocalWins => "local wins",
                            };
                            println!(
                                "  {} Conflict on {} ({}) -- {}",
                                "!!".yellow(), issue.identifier, task.title, winner
                            );
                        }
                    } else if remote_changed {
                        apply_remote_to_local(db, &entry, issue, &remote_hash, quiet)?;
                        stats.pulled_updated += 1;
                    }
                }
            }
            None => {
                create_local_from_remote(db, issue, quiet)?;
                stats.pulled_new += 1;
            }
        }
    }

    // Phase 2: Push local tasks
    let tasks = db.list_all_tasks()?;
    for task in &tasks {
        // Skip closed local tasks on push
        if task.status == Status::Closed {
            stats.skipped_inactive += 1;
            continue;
        }

        let sync_entry = db.get_linear_sync_by_task(task.id)?;

        match sync_entry {
            Some(entry) => {
                let local_hash = mapping::hash_task(task);
                if local_hash != entry.last_local_hash {
                    let state_id = mapping::status_to_linear_state_id(&task.status, &states, config)?;
                    let priority = mapping::priority_to_linear(&task.priority, config);
                    let assignee_linear_id = resolve_assignee(db, task.assignee_id, config, &members)?;
                    let mut label_ids = get_label_ids_for_task(db, task, client, config, &existing_labels)?;
                    for fid in &filter_label_ids {
                        if !label_ids.contains(fid) {
                            label_ids.push(fid.clone());
                        }
                    }

                    match client.update_issue(
                        &entry.linear_issue_id,
                        Some(&task.title),
                        task.description.as_deref(),
                        Some(priority),
                        Some(&state_id),
                        assignee_linear_id.as_deref(),
                        task.due_date.map(|d| d.to_string()).as_deref(),
                        if label_ids.is_empty() { None } else { Some(&label_ids) },
                    ) {
                        Ok(issue) => {
                            let remote_hash = mapping::hash_issue(&issue);
                            db.update_linear_sync(task.id, &local_hash, &remote_hash)?;
                            stats.pushed_updated += 1;
                            if !quiet {
                                println!("  {} Updated {} -> {}", "->".blue(), task.title, issue.identifier);
                            }
                        }
                        Err(e) => stats.errors.push(format!("Failed to update {}: {}", task.id, e)),
                    }
                }
            }
            None => {
                let state_id = mapping::status_to_linear_state_id(&task.status, &states, config)?;
                let priority = mapping::priority_to_linear(&task.priority, config);
                let assignee_linear_id = resolve_assignee(db, task.assignee_id, config, &members)?;
                let mut label_ids = get_label_ids_for_task(db, task, client, config, &existing_labels)?;
                for fid in &filter_label_ids {
                    if !label_ids.contains(fid) {
                        label_ids.push(fid.clone());
                    }
                }

                match client.create_issue(
                    &config.team_id,
                    &task.title,
                    task.description.as_deref(),
                    priority,
                    &state_id,
                    assignee_linear_id.as_deref(),
                    task.due_date.map(|d| d.to_string()).as_deref(),
                    &label_ids,
                ) {
                    Ok(issue) => {
                        let local_hash = mapping::hash_task(task);
                        let remote_hash = mapping::hash_issue(&issue);
                        db.create_linear_sync(task.id, &issue.id, Some(&issue.identifier), &local_hash, &remote_hash)?;
                        stats.pushed_new += 1;
                        if !quiet {
                            println!("  {} Created {} -> {}", "++".green(), task.title, issue.identifier);
                        }
                    }
                    Err(e) => stats.errors.push(format!("Failed to create issue for task {}: {}", task.id, e)),
                }
            }
        }
    }

    Ok(stats)
}

// --- Helper functions ---

fn apply_remote_to_local(
    db: &mut Database,
    entry: &crate::db::LinearSyncEntry,
    issue: &LinearIssue,
    remote_hash: &str,
    quiet: bool,
) -> Result<()> {
    let status = mapping::status_from_linear(&issue.state);
    let priority = mapping::priority_from_linear(issue.priority);
    let tags = labels_to_tags(&issue.labels.nodes);
    let due_date = issue.due_date.as_deref()
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

    db.update_task(
        entry.local_task_id,
        Some(&issue.title),
        issue.description.as_deref(),
        Some(status),
        Some(priority),
        None,
        None,
        Some(due_date),
        Some(tags.as_deref()),
        None,
        None,
        None,
    )?;

    let local_task = db.get_task(entry.local_task_id)?;
    let local_hash = local_task.map(|t| mapping::hash_task(&t)).unwrap_or_default();
    db.update_linear_sync(entry.local_task_id, &local_hash, remote_hash)?;

    if !quiet {
        println!("  {} Updated <- {}", "<-".cyan(), issue.identifier);
    }
    Ok(())
}

fn create_local_from_remote(
    db: &mut Database,
    issue: &LinearIssue,
    quiet: bool,
) -> Result<()> {
    let status = mapping::status_from_linear(&issue.state);
    let priority = mapping::priority_from_linear(issue.priority);
    let tags = labels_to_tags(&issue.labels.nodes);
    let due_date = issue.due_date.as_deref()
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

    let task = db.create_task(
        &issue.title,
        issue.description.as_deref(),
        None,
        priority,
        None,
        due_date,
        tags.as_deref(),
        None,
        None,
        false, // is_knowledge default for Linear sync
        None,  // key_questions default for Linear sync
    )?;

    if status == Status::Closed {
        db.update_task(task.id, None, None, Some(Status::Closed), None, None, None, None, None, None, None, None)?;
    }

    let local_hash = mapping::hash_task(&task);
    let remote_hash = mapping::hash_issue(issue);
    db.create_linear_sync(task.id, &issue.id, Some(&issue.identifier), &local_hash, &remote_hash)?;

    if !quiet {
        println!("  {} Created <- {} ({})", "++".green(), issue.identifier, issue.title);
    }
    Ok(())
}

fn labels_to_tags(labels: &[LinearLabel]) -> Option<String> {
    if labels.is_empty() {
        None
    } else {
        Some(labels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>().join(","))
    }
}

/// Resolve the Linear assignee ID for a task.
/// Tries local assignee mapping first, then falls back to config.default_assignee_id.
fn resolve_assignee(
    db: &Database,
    assignee_id: Option<i64>,
    config: &LinearConfig,
    members: &[super::client::LinearUser],
) -> Result<Option<String>> {
    // Try to match from local assignee
    if let Some(id) = assignee_id {
        if id > 0 {
            if let Some(assignee) = db.get_assignee(id)? {
                let found = mapping::find_linear_user_id(
                    assignee.email.as_deref(),
                    Some(&assignee.name),
                    assignee.github_username.as_deref(),
                    config,
                    members,
                );
                if found.is_some() {
                    return Ok(found);
                }
            }
        }
    }

    // Fall back to default assignee from config
    Ok(config.default_assignee_id.clone())
}

fn get_label_ids_for_task(
    db: &Database,
    task: &crate::models::Task,
    client: &LinearClient,
    config: &LinearConfig,
    existing_labels: &[LinearLabel],
) -> Result<Vec<String>> {
    let mut label_ids = Vec::new();

    // Map epic to label if epic_mode is "label"
    if config.mapping.epic_mode == "label" {
        if let Some(epic_id) = task.epic_id {
            if let Some(epic) = db.get_epic(epic_id)? {
                if let Some(label) = existing_labels.iter().find(|l| l.name == epic.title) {
                    label_ids.push(label.id.clone());
                } else {
                    match client.create_label(&config.team_id, &epic.title) {
                        Ok(label) => label_ids.push(label.id.clone()),
                        Err(e) => {
                            eprintln!("  Warning: could not create label '{}': {}", epic.title, e);
                        }
                    }
                }
            }
        }
    }

    // Also map tags to labels
    if let Some(tags) = &task.tags {
        for tag in tags.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()) {
            if let Some(label) = existing_labels.iter().find(|l| l.name.eq_ignore_ascii_case(tag)) {
                if !label_ids.contains(&label.id) {
                    label_ids.push(label.id.clone());
                }
            } else {
                match client.create_label(&config.team_id, tag) {
                    Ok(label) => {
                        if !label_ids.contains(&label.id) {
                            label_ids.push(label.id.clone());
                        }
                    }
                    Err(_) => {}
                }
            }
        }
    }

    Ok(label_ids)
}
