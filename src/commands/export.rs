use std::fs;
use colored::Colorize;
use crate::commands::{ensure_initialized, SUCCESS_PREFIX};
use crate::error::Result;

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
    
    let mut csv = String::new();
    csv.push_str("id,title,description,status,priority,epic_id,assignee_id,due_date,created_at\n");
    
    for task in tasks {
        let desc = task.description.unwrap_or_default().replace('"', "\"\"");
        let due = task.due_date.map(|d| d.to_string()).unwrap_or_default();
        let epic = task.epic_id.map(|id| id.to_string()).unwrap_or_default();
        let assignee = task.assignee_id.map(|id| id.to_string()).unwrap_or_default();
        let title = task.title.replace('"', "\"\"");
        
        csv.push_str(&format!(
            "{},\"{}\",\"{}\",{},{},{},{},{},{}\n",
            task.id,
            title,
            desc,
            task.status,
            task.priority,
            epic,
            assignee,
            due,
            task.created_at.to_rfc3339()
        ));
    }
    
    if let Some(path) = output {
        fs::write(path, csv)?;
        if !quiet {
            println!("{} Exported to {}", SUCCESS_PREFIX.green(), path.cyan());
        }
    } else {
        println!("{}", csv);
    }
    
    Ok(())
}
