use crate::cli::OutputFormat;
use crate::commands::{ensure_initialized, tid, INFO_PREFIX, SUCCESS_PREFIX};
use crate::error::Result;
use colored::Colorize;

pub fn show(task_id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;

    let task = db
        .get_task(task_id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
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
            println!(
                "{} Dependency tree for task {}: {}",
                INFO_PREFIX.blue(),
                tid(task_id),
                task.title.bold()
            );
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
                        println!("    {} {}: {}", status, tid(*id), t.title);
                    } else {
                        println!("    ? {}: (not found)", tid(*id));
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
                        println!("    {} {}: {}", status, tid(*id), t.title);
                    } else {
                        println!("    ? {}: (not found)", tid(*id));
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
        println!(
            "{} Task {} no longer blocks task {}",
            SUCCESS_PREFIX.green(),
            tid(task_id),
            tid(blocked_task_id)
        );
    }
    Ok(())
}
