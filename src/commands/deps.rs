use colored::Colorize;
use crate::commands::{ensure_initialized, SUCCESS_PREFIX, ERROR_PREFIX, INFO_PREFIX, WARNING_PREFIX};
use crate::cli::OutputFormat;
use crate::error::Result;

pub fn show(task_id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    
    let task = db.get_task(task_id)?.ok_or_else(|| crate::error::MyceliumError::NotFound {
        entity: "task".to_string(),
        id: task_id.to_string(),
    })?;
    
    let chain = db.get_all_dependencies(task_id)?;
    
    if quiet {
        for id in &chain.all_dependencies {
            println!("{}", id);
        }
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&chain)?),
        OutputFormat::Table => {
            println!("{} Dependency tree for task #{}: {}", INFO_PREFIX.blue(), task_id, task.title.bold());
            println!();
            
            if chain.blocked_by.is_empty() {
                println!("  Not blocked by any tasks.");
            } else {
                println!("  Blocked by (must complete first):");
                for id in &chain.blocked_by {
                    if let Ok(Some(t)) = db.get_task(*id) {
                        let status = if t.status == crate::models::Status::Closed {
                            "✅".green()
                        } else {
                            "⭕".red()
                        };
                        println!("    {} #{}: {}", status, id, t.title);
                    } else {
                        println!("    ? #{}: (not found)", id);
                    }
                }
            }
            
            println!();
            
            if chain.blocks.is_empty() {
                println!("  Not blocking any tasks.");
            } else {
                println!("  Blocks (waiting on this):");
                for id in &chain.blocks {
                    if let Ok(Some(t)) = db.get_task(*id) {
                        let status = if t.status == crate::models::Status::Closed {
                            "✅".green()
                        } else {
                            "⭕".red()
                        };
                        println!("    {} #{}: {}", status, id, t.title);
                    } else {
                        println!("    ? #{}: (not found)", id);
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn unlink(task_id: i64, blocked_task_id: i64, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    
    db.remove_dependency(blocked_task_id, task_id)?;
    
    if !quiet {
        println!("{} Task #{} no longer blocks task #{}", SUCCESS_PREFIX.green(), task_id, blocked_task_id);
    }
    Ok(())
}
