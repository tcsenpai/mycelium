use chrono::{NaiveDate, Local, Duration};
use colored::Colorize;
use comfy_table::{Table, ContentArrangement};
use crate::commands::{ensure_initialized, SUCCESS_PREFIX, ERROR_PREFIX, INFO_PREFIX, WARNING_PREFIX};
use crate::cli::OutputFormat;
use crate::models::{Priority, Status};
use crate::models::external_ref::parse_github_ref;
use crate::error::Result;
use std::fs;

pub fn create(
    title: &str,
    description: Option<&str>,
    epic_id: Option<i64>,
    priority: &str,
    assignee_id: Option<i64>,
    due: Option<&str>,
    tags: Option<&str>,
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
    
    let task = db.create_task(title, description, epic_id, priority_enum, assignee_id, due_date, final_tags.as_deref())?;
    
    if quiet {
        println!("{}", task.id);
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&task)?),
        OutputFormat::Table => {
            println!("{} Created task #{}: {}", SUCCESS_PREFIX.green(), task.id.to_string().cyan(), task.title.bold());
        }
    }
    Ok(())
}

pub fn list(
    epic_id: Option<i64>,
    status: Option<&str>,
    priority: Option<&str>,
    assignee_id: Option<i64>,
    blocked: bool,
    overdue: bool,
    tag: Option<&str>,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let db = ensure_initialized()?;
    
    let status_enum: Option<Status> = status.map(|s| s.parse()).transpose()?;
    let priority_enum: Option<Priority> = priority.map(|p| p.parse()).transpose()?;
    
    let tasks = db.list_tasks(epic_id, status_enum, priority_enum, assignee_id, blocked, overdue, tag)?;
    
    if quiet {
        for task in &tasks {
            println!("{}", task.id);
        }
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&tasks)?),
        OutputFormat::Table => {
            if tasks.is_empty() {
                println!("{} No tasks found.", INFO_PREFIX.blue());
                return Ok(());
            }
            
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["ID", "Title", "Status", "Priority", "Epic", "Due", "Tags"]);
            
            let task_count = tasks.len();
            for task in tasks {
                let epic = task.epic_id.map(|id| format!("#{}", id));
                let due = task.due_date.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string());
                let due_str = if task.is_overdue() {
                    due.red().to_string()
                } else {
                    due
                };
                let tags = task.tags.unwrap_or_else(|| "-".to_string());
                
                table.add_row(vec![
                    task.id.to_string(),
                    task.title.clone(),
                    format!("{} {}", task.status.emoji(), task.status),
                    format!("{} {}", task.priority.emoji(), task.priority),
                    epic.unwrap_or_else(|| "-".to_string()),
                    due_str,
                    tags,
                ]);
            }
            
            println!("{}", table);
            println!("\nShowing {} task(s)", task_count);
        }
    }
    Ok(())
}

pub fn show(id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let task = db.get_task(id)?.ok_or_else(|| crate::error::MyceliumError::NotFound {
        entity: "task".to_string(),
        id: id.to_string(),
    })?;
    
    let epic_title = task.epic_id.and_then(|eid| db.get_epic(eid).ok().flatten().map(|e| e.title));
    let assignee = task.assignee_id.and_then(|aid| db.get_assignee(aid).ok().flatten().map(|a| a.name));
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
            println!("{} Task #{}", INFO_PREFIX.blue(), task.id.to_string().cyan().bold());
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
                    println!("  Due: {} {}", due.to_string().red().bold(), "OVERDUE".red().bold());
                } else {
                    println!("  Due: {}", due);
                }
            }
            if let Some(tags) = &task.tags {
                println!("  Tags: {}", tags.yellow());
            }
            println!("  Created: {}", task.created_at.format("%Y-%m-%d %H:%M"));
            println!();
            
            if !blocking.is_empty() {
                println!("  Blocked by: {}", blocking.iter().map(|id| format!("#{}", id)).collect::<Vec<_>>().join(", "));
            }
            if !blocked_by.is_empty() {
                println!("  Blocks: {}", blocked_by.iter().map(|id| format!("#{}", id)).collect::<Vec<_>>().join(", "));
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

pub fn update(
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
    priority: Option<&str>,
    epic_id: Option<i64>,
    assignee_id: Option<i64>,
    due: Option<&str>,
    tags: Option<&str>,
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
    
    let task = db.update_task(id, title, description, status_enum, priority_enum, epic_opt, assignee_opt, due_date, tags_opt)?;
    
    if quiet {
        println!("{}", task.id);
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&task)?),
        OutputFormat::Table => {
            println!("{} Updated task #{}: {}", SUCCESS_PREFIX.green(), task.id.to_string().cyan(), task.title.bold());
        }
    }
    Ok(())
}

pub fn delete(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let task = db.get_task(id)?.ok_or_else(|| crate::error::MyceliumError::NotFound {
        entity: "task".to_string(),
        id: id.to_string(),
    })?;
    
    if !force {
        if !crate::commands::confirm(&format!("Delete task #{}: '{}'", id, task.title)) {
            println!("Cancelled.");
            return Ok(());
        }
    }
    
    db.delete_task(id)?;
    
    if !quiet {
        println!("{} Deleted task #{}: {}", SUCCESS_PREFIX.green(), id.to_string().cyan(), task.title);
    }
    Ok(())
}

pub fn assign(task_id: i64, assignee_id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let assignee_opt = if assignee_id == 0 { None } else { Some(assignee_id) };
    let _task = db.update_task(task_id, None, None, None, None, None, Some(assignee_opt), None, None)?;
    
    if !quiet {
        if assignee_id == 0 {
            println!("{} Unassigned task #{}", SUCCESS_PREFIX.green(), task_id);
        } else {
            println!("{} Assigned task #{} to assignee #{}", SUCCESS_PREFIX.green(), task_id, assignee_id);
        }
    }
    Ok(())
}

pub fn link_github_issue(task_id: i64, reference: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let (owner, repo, number) = parse_github_ref(reference)
        .ok_or_else(|| crate::error::MyceliumError::InvalidGitHubRef(reference.to_string()))?;
    
    let _ext_ref = db.add_external_ref(task_id, crate::models::ExternalRefType::GitHubIssue, 
        &format!("{}/{}/{}", owner, repo, number))?;
    
    if !quiet {
        println!("{} Linked task #{} to GitHub issue {}", SUCCESS_PREFIX.green(), task_id, reference.cyan());
    }
    Ok(())
}

pub fn link_github_pr(task_id: i64, reference: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let (owner, repo, number) = parse_github_ref(reference)
        .ok_or_else(|| crate::error::MyceliumError::InvalidGitHubRef(reference.to_string()))?;
    
    let _ext_ref = db.add_external_ref(task_id, crate::models::ExternalRefType::GitHubPr, 
        &format!("{}/{}/{}", owner, repo, number))?;
    
    if !quiet {
        println!("{} Linked task #{} to GitHub PR {}", SUCCESS_PREFIX.green(), task_id, reference.cyan());
    }
    Ok(())
}

pub fn link_url(task_id: i64, url: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let _ext_ref = db.add_external_ref(task_id, crate::models::ExternalRefType::Url, url)?;
    
    if !quiet {
        println!("{} Linked task #{} to URL {}", SUCCESS_PREFIX.green(), task_id, url.cyan());
    }
    Ok(())
}

pub fn link_blocks(task_id: i64, blocked_id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    db.add_dependency(blocked_id, task_id)?;
    
    if !quiet {
        println!("{} Task #{} now blocks task #{}", SUCCESS_PREFIX.green(), task_id, blocked_id);
    }
    Ok(())
}

pub fn unlink_ref(ref_id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    db.remove_external_ref(ref_id)?;
    
    if !quiet {
        println!("{} Removed external reference {}", SUCCESS_PREFIX.green(), ref_id);
    }
    Ok(())
}

pub fn close(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let _task = db.get_task(id)?.ok_or_else(|| crate::error::MyceliumError::NotFound {
        entity: "task".to_string(),
        id: id.to_string(),
    })?;
    
    let blockers = db.get_open_blockers(id)?;
    if !blockers.is_empty() && !force {
        println!("{} Cannot close task #{} - blocked by:", ERROR_PREFIX.red(), id);
        for blocker in &blockers {
            println!("  - #{}: {}", blocker.id, blocker.title);
        }
        println!("\nUse --force to close anyway (or resolve the blockers first).");
        return Ok(());
    }
    
    let updated = db.update_task(id, None, None, Some(Status::Closed), None, None, None, None, None)?;
    
    if !quiet {
        println!("{} Closed task #{}: {}", SUCCESS_PREFIX.green(), id, updated.title);
    }
    Ok(())
}

pub fn reopen(id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let updated = db.update_task(id, None, None, Some(Status::Open), None, None, None, None, None)?;
    
    if !quiet {
        println!("{} Reopened task #{}: {}", SUCCESS_PREFIX.green(), id, updated.title);
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
                            .ok_or_else(|| crate::error::MyceliumError::InvalidGitHubRef(ext_ref.reference.clone()))?;
                        db.add_external_ref(task.id, crate::models::ExternalRefType::GitHubIssue, 
                            &format!("{}/{}/{}", owner, repo, number))?;
                    }
                    "github-pr" => {
                        let (owner, repo, number) = parse_github_ref(&ext_ref.reference)
                            .ok_or_else(|| crate::error::MyceliumError::InvalidGitHubRef(ext_ref.reference.clone()))?;
                        db.add_external_ref(task.id, crate::models::ExternalRefType::GitHubPr, 
                            &format!("{}/{}/{}", owner, repo, number))?;
                    }
                    "url" => {
                        db.add_external_ref(task.id, crate::models::ExternalRefType::Url, &ext_ref.reference)?;
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
                println!("{} Created {} task(s)", SUCCESS_PREFIX.green(), created_ids.len());
                for id in &created_ids {
                    println!("  - Task #{}", id);
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
fn apply_template(template: &str, priority: &str, tags: Option<&str>) -> Result<(String, Option<String>)> {
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
                    
                    let tpl_tags = template_table
                        .get("tags")
                        .and_then(|v| v.as_str());
                    
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
