use colored::Colorize;
use comfy_table::{Table, ContentArrangement};
use crate::commands::{ensure_initialized, SUCCESS_PREFIX, ERROR_PREFIX, INFO_PREFIX};
use crate::cli::OutputFormat;
use crate::models::Status;
use crate::error::Result;

pub fn create(title: &str, description: Option<&str>, format: &OutputFormat, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    let epic = db.create_epic(title, description)?;
    
    if quiet {
        println!("{}", epic.id);
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&epic)?),
        OutputFormat::Table => {
            println!("{} Created epic #{}: {}", SUCCESS_PREFIX.green(), epic.id.to_string().cyan(), epic.title.bold());
        }
    }
    Ok(())
}

pub fn list(format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let epics = db.list_epics_with_summary()?;
    
    if quiet {
        for summary in &epics {
            println!("{}", summary.epic.id);
        }
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&epics)?),
        OutputFormat::Table => {
            if epics.is_empty() {
                println!("{} No epics found. Create one with `myc epic create --title \"...\"`", INFO_PREFIX.blue());
                return Ok(());
            }
            
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["ID", "Title", "Status", "Tasks", "Progress"]);
            
            for summary in epics {
                let status_emoji = summary.epic.status.emoji();
                let progress = if summary.total_tasks == 0 {
                    "—".to_string()
                } else {
                    let pct = (summary.closed_tasks * 100 / summary.total_tasks) as u8;
                    format!("{}% ({} / {})", pct, summary.closed_tasks, summary.total_tasks)
                };
                
                table.add_row(vec![
                    summary.epic.id.to_string(),
                    summary.epic.title,
                    format!("{} {}", status_emoji, summary.epic.status),
                    summary.total_tasks.to_string(),
                    progress,
                ]);
            }
            
            println!("{}", table);
        }
    }
    Ok(())
}

pub fn show(id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let epic = db.get_epic(id)?.ok_or_else(|| crate::error::MyceliumError::NotFound {
        entity: "epic".to_string(),
        id: id.to_string(),
    })?;
    
    // Get tasks for this epic
    let tasks = db.list_tasks(Some(id), None, None, None, false, false)?;
    
    if quiet {
        println!("{}", epic.id);
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => {
            #[derive(serde::Serialize)]
            struct EpicWithTasks {
                #[serde(flatten)]
                epic: crate::models::Epic,
                tasks: Vec<crate::models::Task>,
            }
            let data = EpicWithTasks { epic, tasks };
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
        OutputFormat::Table => {
            println!("{} Epic #{}", INFO_PREFIX.blue(), epic.id.to_string().cyan().bold());
            println!("  Title: {}", epic.title.bold());
            if let Some(desc) = &epic.description {
                println!("  Description: {}", desc);
            }
            println!("  Status: {} {}", epic.status.emoji(), epic.status);
            println!("  Created: {}", epic.created_at.format("%Y-%m-%d %H:%M"));
            println!();
            
            if tasks.is_empty() {
                println!("  No tasks in this epic.");
            } else {
                println!("  Tasks ({}):", tasks.len());
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

pub fn update(id: i64, title: Option<&str>, description: Option<&str>, status: Option<&str>, format: &OutputFormat, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let status_enum: Option<Status> = status.map(|s| s.parse()).transpose()?;
    let epic = db.update_epic(id, title, description, status_enum)?;
    
    if quiet {
        println!("{}", epic.id);
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&epic)?),
        OutputFormat::Table => {
            println!("{} Updated epic #{}: {}", SUCCESS_PREFIX.green(), epic.id.to_string().cyan(), epic.title.bold());
        }
    }
    Ok(())
}

pub fn delete(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    let epic = db.get_epic(id)?.ok_or_else(|| crate::error::MyceliumError::NotFound {
        entity: "epic".to_string(),
        id: id.to_string(),
    })?;
    
    // Check for tasks
    let tasks = db.list_tasks(Some(id), None, None, None, false, false)?;
    
    if !force && !tasks.is_empty() {
        println!("{} Epic '{}' has {} task(s).", WARNING_PREFIX.yellow(), epic.title, tasks.len());
        if !crate::commands::confirm(&format!("Delete epic #{} and all its tasks?", id)) {
            println!("Cancelled.");
            return Ok(());
        }
    }
    
    db.delete_epic(id)?;
    
    if !quiet {
        println!("{} Deleted epic #{}: {}", SUCCESS_PREFIX.green(), id.to_string().cyan(), epic.title);
    }
    Ok(())
}

use crate::commands::WARNING_PREFIX;
