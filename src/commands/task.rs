use chrono::NaiveDate;
use colored::Colorize;
use comfy_table::{Table, ContentArrangement};
use crate::commands::{ensure_initialized, SUCCESS_PREFIX, ERROR_PREFIX, INFO_PREFIX, WARNING_PREFIX};
use crate::cli::OutputFormat;
use crate::models::{Priority, Status};
use crate::models::external_ref::parse_github_ref;
use crate::error::Result;

pub fn create(
    title: &str,
    description: Option<&str>,
    epic_id: Option<i64>,
    priority: &str,
    assignee_id: Option<i64>,
    due: Option<&str>,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let priority_enum: Priority = priority.parse()?;
    let due_date = due.map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| crate::error::MyceliumError::InvalidDate(due.unwrap().to_string()))?;
    
    let task = db.create_task(title, description, epic_id, priority_enum, assignee_id, due_date)?;
    
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
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let db = ensure_initialized()?;
    
    let status_enum: Option<Status> = status.map(|s| s.parse()).transpose()?;
    let priority_enum: Option<Priority> = priority.map(|p| p.parse()).transpose()?;
    
    let tasks = db.list_tasks(epic_id, status_enum, priority_enum, assignee_id, blocked, overdue)?;
    
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
            table.set_header(vec!["ID", "Title", "Status", "Priority", "Epic", "Due"]);
            
            let task_count = tasks.len();
            for task in tasks {
                let epic = task.epic_id.map(|id| format!("#{}", id));
                let due = task.due_date.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string());
                let due_str = if task.is_overdue() {
                    due.red().to_string()
                } else {
                    due
                };
                
                table.add_row(vec![
                    task.id.to_string(),
                    task.title.clone(),
                    format!("{} {}", task.status.emoji(), task.status),
                    format!("{} {}", task.priority.emoji(), task.priority),
                    epic.unwrap_or_else(|| "-".to_string()),
                    due_str,
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
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let status_enum = status.map(|s| s.parse::<Status>()).transpose()?;
    let priority_enum = priority.map(|p| p.parse::<Priority>()).transpose()?;
    let due_date: Option<Option<NaiveDate>> = match due {
        Some(d) => {
            let date = NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .map_err(|_| crate::error::MyceliumError::InvalidDate(d.to_string()))?;
            Some(Some(date))
        }
        None => None,
    };
    
    let epic_opt = epic_id.map(|e| if e == 0 { None } else { Some(e) });
    let assignee_opt = assignee_id.map(|a| if a == 0 { None } else { Some(a) });
    
    let task = db.update_task(id, title, description, status_enum, priority_enum, epic_opt, assignee_opt, due_date)?;
    
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
    let _task = db.update_task(task_id, None, None, None, None, None, Some(assignee_opt), None)?;
    
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
    
    let updated = db.update_task(id, None, None, Some(Status::Closed), None, None, None, None)?;
    
    if !quiet {
        println!("{} Closed task #{}: {}", SUCCESS_PREFIX.green(), id, updated.title);
    }
    Ok(())
}

pub fn reopen(id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let updated = db.update_task(id, None, None, Some(Status::Open), None, None, None, None)?;
    
    if !quiet {
        println!("{} Reopened task #{}: {}", SUCCESS_PREFIX.green(), id, updated.title);
    }
    Ok(())
}
