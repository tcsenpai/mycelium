use crate::cli::OutputFormat;
use crate::commands::{ensure_initialized, ERROR_PREFIX, INFO_PREFIX, SUCCESS_PREFIX};
use crate::error::Result;
use crate::models::Status;
use colored::Colorize;
use comfy_table::{ContentArrangement, Table};

pub fn create(
    title: &str,
    description: Option<&str>,
    notes: Option<&str>,
    user_info: Option<&str>,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;
    let epic = db.create_epic(title, description, notes, user_info)?;

    if quiet {
        println!("{}", epic.id);
        return Ok(());
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&epic)?),
        OutputFormat::Table => {
            println!(
                "{} Created epic #{}: {}",
                SUCCESS_PREFIX.green(),
                epic.id.to_string().cyan(),
                epic.title.bold()
            );
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
                println!(
                    "{} No epics found. Create one with `myc epic create --title \"...\"`",
                    INFO_PREFIX.blue()
                );
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
                    format!(
                        "{}% ({} / {})",
                        pct, summary.closed_tasks, summary.total_tasks
                    )
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
    let epic = db
        .get_epic(id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "epic".to_string(),
            id: id.to_string(),
        })?;

    // Get tasks for this epic
    let tasks = db.list_tasks(Some(id), None, None, None, false, false, None)?;

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
            println!(
                "{} Epic #{}",
                INFO_PREFIX.blue(),
                epic.id.to_string().cyan().bold()
            );
            println!("  Title: {}", epic.title.bold());
            if let Some(desc) = &epic.description {
                println!("  Description: {}", desc);
            }
            println!("  Status: {} {}", epic.status.emoji(), epic.status);
            if let Some(notes) = &epic.notes {
                println!("  Notes: {}", notes);
            }
            if let Some(user_info) = &epic.user_info {
                println!("  User Info: {}", user_info);
            }
            if let Some(agent_questions) = &epic.agent_questions {
                println!("  Agent Questions: {}", agent_questions.cyan());
            }
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

pub fn update(
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
    notes: Option<&str>,
    user_info: Option<&str>,
    agent_questions: Option<&str>,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;

    let status_enum: Option<Status> = status.map(|s| s.parse()).transpose()?;
    let notes_opt = notes.map(|n| if n == "-" { None } else { Some(n) });
    let user_info_opt = user_info.map(|u| if u == "-" { None } else { Some(u) });
    let agent_questions_opt = agent_questions.map(|q| if q == "-" { None } else { Some(q) });
    let epic = db.update_epic(
        id,
        title,
        description,
        status_enum,
        notes_opt,
        user_info_opt,
        agent_questions_opt,
    )?;

    if quiet {
        println!("{}", epic.id);
        return Ok(());
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&epic)?),
        OutputFormat::Table => {
            println!(
                "{} Updated epic #{}: {}",
                SUCCESS_PREFIX.green(),
                epic.id.to_string().cyan(),
                epic.title.bold()
            );
        }
    }
    Ok(())
}

pub fn delete(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let epic = db
        .get_epic(id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "epic".to_string(),
            id: id.to_string(),
        })?;

    // Check for tasks
    let tasks = db.list_tasks(Some(id), None, None, None, false, false, None)?;

    if !force && !tasks.is_empty() {
        println!(
            "{} Epic '{}' has {} linked task(s).",
            WARNING_PREFIX.yellow(),
            epic.title,
            tasks.len()
        );
        if !crate::commands::confirm(&format!(
            "Delete epic #{} and detach those tasks from the epic?",
            id
        )) {
            println!("Cancelled.");
            return Ok(());
        }
    }

    db.delete_epic(id)?;

    if !quiet {
        println!(
            "{} Deleted epic #{}: {}",
            SUCCESS_PREFIX.green(),
            id.to_string().cyan(),
            epic.title
        );
    }
    Ok(())
}

pub fn add_note(epic_id: i64, content: &str, format: &OutputFormat, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let epic = db
        .get_epic(epic_id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "epic".to_string(),
            id: epic_id.to_string(),
        })?;

    let note = db.add_epic_note(epic_id, content)?;

    if quiet {
        println!("{}", note.id);
        return Ok(());
    }

    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&note)?),
        OutputFormat::Table => {
            println!(
                "{} Added note #{} to epic #{}: {}",
                SUCCESS_PREFIX.green(),
                note.id.to_string().cyan(),
                epic_id.to_string().cyan(),
                epic.title.bold()
            );
        }
    }
    Ok(())
}

pub fn show_notes(epic_id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;

    let epic = db
        .get_epic(epic_id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "epic".to_string(),
            id: epic_id.to_string(),
        })?;

    let notes = db.list_epic_notes(epic_id)?;

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
                "{} Notes for epic #{}: {}",
                INFO_PREFIX.blue(),
                epic_id.to_string().cyan(),
                epic.title.bold()
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

use crate::commands::WARNING_PREFIX;
