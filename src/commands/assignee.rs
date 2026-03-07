use colored::Colorize;
use comfy_table::{Table, ContentArrangement};
use crate::commands::{ensure_initialized, SUCCESS_PREFIX, ERROR_PREFIX, INFO_PREFIX, WARNING_PREFIX};
use crate::cli::OutputFormat;
use crate::error::Result;

pub fn create(name: &str, email: Option<&str>, github: Option<&str>, format: &OutputFormat, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    let assignee = db.create_assignee(name, email, github)?;
    
    if quiet {
        println!("{}", assignee.id);
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&assignee)?),
        OutputFormat::Table => {
            println!("{} Created assignee #{}: {}", SUCCESS_PREFIX.green(), assignee.id.to_string().cyan(), assignee.name.bold());
        }
    }
    Ok(())
}

pub fn list(format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let assignees = db.list_assignees_with_stats()?;
    
    if quiet {
        for stat in &assignees {
            println!("{}", stat.assignee.id);
        }
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&assignees)?),
        OutputFormat::Table => {
            if assignees.is_empty() {
                println!("{} No assignees found.", INFO_PREFIX.blue());
                return Ok(());
            }
            
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["ID", "Name", "Email", "GitHub", "Tasks (Open/Total)"]);
            
            for stat in assignees {
                let email = stat.assignee.email.unwrap_or_else(|| "-".to_string());
                let github = stat.assignee.github_username.unwrap_or_else(|| "-".to_string());
                let tasks = format!("{} / {}", stat.open_tasks, stat.total_tasks);
                
                table.add_row(vec![
                    stat.assignee.id.to_string(),
                    stat.assignee.name,
                    email,
                    github,
                    tasks,
                ]);
            }
            
            println!("{}", table);
        }
    }
    Ok(())
}

pub fn show(id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let assignee = db.get_assignee(id)?.ok_or_else(|| crate::error::MyceliumError::NotFound {
        entity: "assignee".to_string(),
        id: id.to_string(),
    })?;
    
    // Get tasks for this assignee
    let tasks = db.list_tasks(None, None, None, Some(id), false, false)?;
    
    if quiet {
        println!("{}", assignee.id);
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => {
            let data = serde_json::json!({
                "assignee": assignee,
                "tasks": tasks,
            });
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        OutputFormat::Table => {
            println!("{} Assignee #{}", INFO_PREFIX.blue(), assignee.id.to_string().cyan().bold());
            println!("  Name: {}", assignee.name.bold());
            if let Some(email) = &assignee.email {
                println!("  Email: {}", email);
            }
            if let Some(github) = &assignee.github_username {
                println!("  GitHub: @{}", github);
            }
            println!();
            
            if tasks.is_empty() {
                println!("  No assigned tasks.");
            } else {
                println!("  Assigned tasks ({}):", tasks.len());
                let mut table = Table::new();
                table.set_header(vec!["ID", "Title", "Status", "Priority"]);
                
                for task in tasks {
                    table.add_row(vec![
                        task.id.to_string(),
                        task.title,
                        format!("{} {}", task.status.emoji(), task.status),
                        format!("{} {}", task.priority.emoji(), task.priority),
                    ]);
                }
                println!("{}", table);
            }
        }
    }
    Ok(())
}

pub fn delete(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let assignee = db.get_assignee(id)?.ok_or_else(|| crate::error::MyceliumError::NotFound {
        entity: "assignee".to_string(),
        id: id.to_string(),
    })?;
    
    // Check for assigned tasks
    let tasks = db.list_tasks(None, None, None, Some(id), false, false)?;
    
    if !force && !tasks.is_empty() {
        println!("{} Assignee '{}' has {} task(s) assigned.", WARNING_PREFIX.yellow(), assignee.name, tasks.len());
        if !crate::commands::confirm(&format!("Delete assignee #{}? Tasks will be unassigned.", id)) {
            println!("Cancelled.");
            return Ok(());
        }
    }
    
    db.delete_assignee(id)?;
    
    if !quiet {
        println!("{} Deleted assignee #{}: {}", SUCCESS_PREFIX.green(), id.to_string().cyan(), assignee.name);
    }
    Ok(())
}
