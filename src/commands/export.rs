use crate::commands::{ensure_initialized, SUCCESS_PREFIX};
use crate::error::Result;
use colored::Colorize;
use std::fs;

pub fn json(output: Option<&str>, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;

    // Collect all data
    let epics = db.list_epics()?;
    let tasks = db.list_tasks(None, None, None, None, false, false, None)?;
    let assignees = db.list_assignees()?;
    let summary = db.get_summary()?;

    let data = serde_json::json!({
        "summary": summary,
        "epics": epics,
        "tasks": tasks,
        "assignees": assignees,
        "exported_at": chrono::Local::now().to_rfc3339(),
    });

    let json_str = serde_json::to_string_pretty(&data)?;

    if let Some(path) = output {
        fs::write(path, json_str)?;
        if !quiet {
            println!("{} Exported to {}", SUCCESS_PREFIX.green(), path.cyan());
        }
    } else {
        println!("{}", json_str);
    }

    Ok(())
}

pub fn csv(output: Option<&str>, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let tasks = db.list_tasks(None, None, None, None, false, false, None)?;

    // Use the `csv` crate instead of hand-rolled string formatting: it
    // correctly quotes/escapes embedded commas, quotes, AND newlines (the
    // previous manual `"{}"` interpolation broke rows whenever a title or
    // description contained a literal newline). Field set mirrors the JSON
    // export's Task fields exactly (see `json()` above / Task in
    // mycelium-core/src/models/task.rs) so no data is silently dropped.
    let to_export_err = |e: csv::Error| crate::error::MyceliumError::Export(e.to_string());

    let mut buf = Vec::new();
    {
        let mut wtr = csv::Writer::from_writer(&mut buf);

        wtr.write_record([
            "id",
            "title",
            "description",
            "status",
            "priority",
            "epic_id",
            "assignee_id",
            "due_date",
            "tags",
            "notes",
            "user_info",
            "agent_questions",
            "created_at",
            "updated_at",
        ])
        .map_err(to_export_err)?;

        for task in tasks {
            wtr.write_record([
                task.id.to_string(),
                task.title,
                task.description.unwrap_or_default(),
                task.status.to_string(),
                task.priority.to_string(),
                task.epic_id.map(|id| id.to_string()).unwrap_or_default(),
                task.assignee_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                task.due_date.map(|d| d.to_string()).unwrap_or_default(),
                task.tags.unwrap_or_default(),
                task.notes.unwrap_or_default(),
                task.user_info.unwrap_or_default(),
                task.agent_questions.unwrap_or_default(),
                task.created_at.to_rfc3339(),
                task.updated_at.to_rfc3339(),
            ])
            .map_err(to_export_err)?;
        }

        wtr.flush().map_err(|e| {
            crate::error::MyceliumError::Export(format!("failed to flush CSV writer: {}", e))
        })?;
    }
    let csv_str = String::from_utf8(buf).map_err(|e| {
        crate::error::MyceliumError::Export(format!("CSV export produced invalid UTF-8: {}", e))
    })?;

    if let Some(path) = output {
        fs::write(path, &csv_str)?;
        if !quiet {
            println!("{} Exported to {}", SUCCESS_PREFIX.green(), path.cyan());
        }
    } else {
        print!("{}", csv_str);
    }

    Ok(())
}
