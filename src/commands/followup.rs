use colored::Colorize;
use comfy_table::{ContentArrangement, Table};

use crate::cli::OutputFormat;
use crate::commands::{
    confirm, ensure_initialized, fid, tid, INFO_PREFIX, SUCCESS_PREFIX, WARNING_PREFIX,
};
use crate::error::{MyceliumError, Result};
use crate::models::{FollowupStatus, Priority};

pub fn add(body: &str, title: Option<&str>, format: &OutputFormat, quiet: bool) -> Result<()> {
    let body = body.trim();
    if body.is_empty() {
        return Err(MyceliumError::InvalidInput(
            "Follow-up body cannot be empty".to_string(),
        ));
    }
    let mut db = ensure_initialized()?;
    let fu = db.create_followup(body, title.map(str::trim).filter(|s| !s.is_empty()))?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&fu)?);
        }
        OutputFormat::Table => {
            if !quiet {
                println!(
                    "{} Added follow-up {}: {}",
                    SUCCESS_PREFIX.green(),
                    fid(fu.id),
                    fu.display_title()
                );
            } else {
                println!("{}", fu.id);
            }
        }
    }
    Ok(())
}

pub fn list(
    status: Option<&str>,
    all: bool,
    open: bool,
    closed: bool,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    // Reject conflicting flags BEFORE resolving so -o + -c doesn't silently
    // resolve to "active".
    if open && closed {
        return Err(MyceliumError::InvalidInput(
            "Cannot use -o and -c together — pick one".to_string(),
        ));
    }

    // Flag precedence: explicit --status wins, then -o/-c, then -a (default).
    let effective: Option<&str> = match (status, open, closed, all) {
        (Some(s), _, _, _) => Some(s),
        (None, true, false, _) => Some("active"),
        (None, false, true, _) => Some("closed"),
        // default OR explicit -a → all
        _ => Some("all"),
    };

    let db = ensure_initialized()?;
    let items = db.list_followups(effective)?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&items)?);
            return Ok(());
        }
        OutputFormat::Table => {}
    }

    if items.is_empty() {
        if !quiet {
            println!("{} No follow-ups.", INFO_PREFIX.blue());
        }
        return Ok(());
    }

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["ID", "Status", "Title", "Created"]);

    for fu in &items {
        table.add_row(vec![
            fid(fu.id).dimmed().to_string(),
            format!("{} {}", fu.status.emoji(), fu.status),
            fu.display_title(),
            fu.created_at.format("%Y-%m-%d %H:%M").to_string(),
        ]);
    }

    if !quiet {
        println!("{}", table);
        println!("{} {} follow-up(s)", INFO_PREFIX.blue(), items.len());
    }
    Ok(())
}

pub fn show(id: i64, format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let fu = db
        .get_followup(id)?
        .ok_or_else(|| MyceliumError::NotFound {
            entity: "followup".to_string(),
            id: id.to_string(),
        })?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&fu)?);
            return Ok(());
        }
        OutputFormat::Table => {}
    }

    if quiet {
        println!("{}", fu.body);
        return Ok(());
    }

    println!("{} Follow-up {}", INFO_PREFIX.blue(), fid(fu.id));
    if let Some(t) = &fu.title {
        if !t.is_empty() {
            println!("  Title:   {}", t.bold());
        }
    }
    println!("  Status:  {} {}", fu.status.emoji(), fu.status);
    println!("  Created: {}", fu.created_at.format("%Y-%m-%d %H:%M"));
    if let Some(closed) = fu.closed_at {
        println!("  Closed:  {}", closed.format("%Y-%m-%d %H:%M"));
    }
    if let Some(reason) = &fu.closure_reason {
        println!("  Reason:  {}", reason);
    }
    println!("\n{}", fu.body);
    Ok(())
}

pub fn next(format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let fu = db.next_followup()?;

    match (format, fu) {
        (OutputFormat::Json, Some(fu)) => {
            println!("{}", serde_json::to_string_pretty(&fu)?);
        }
        (OutputFormat::Json, None) => {
            println!("null");
        }
        (OutputFormat::Table, Some(fu)) => {
            if quiet {
                println!("{}", fu.id);
            } else {
                println!(
                    "{} Next follow-up: {} — {}",
                    INFO_PREFIX.blue(),
                    fid(fu.id),
                    fu.display_title()
                );
                if !fu.body.trim().is_empty() {
                    println!("\n{}", fu.body);
                }
            }
        }
        (OutputFormat::Table, None) => {
            if !quiet {
                println!("{} No active follow-ups.", INFO_PREFIX.blue());
            }
        }
    }
    Ok(())
}

pub fn set_status(
    id: i64,
    status: FollowupStatus,
    reason: Option<&str>,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;
    let updated = db.update_followup_status(id, status, reason)?;

    if !quiet {
        println!(
            "{} Follow-up {} → {} {}",
            SUCCESS_PREFIX.green(),
            fid(updated.id),
            updated.status.emoji(),
            updated.status
        );
    }
    Ok(())
}

pub fn edit(
    id: i64,
    body: Option<&str>,
    title: Option<&str>,
    clear_title: bool,
    quiet: bool,
) -> Result<()> {
    let mut db = ensure_initialized()?;
    let updated = db.update_followup_body(id, body, title, clear_title)?;

    if !quiet {
        println!(
            "{} Follow-up {} updated",
            SUCCESS_PREFIX.green(),
            fid(updated.id)
        );
    }
    Ok(())
}

/// Append text to the body (preserves existing content, separated by a blank line).
pub fn append(id: i64, text: &str, quiet: bool) -> Result<()> {
    let text = text.trim();
    if text.is_empty() {
        return Err(MyceliumError::InvalidInput(
            "Append text cannot be empty".to_string(),
        ));
    }
    let mut db = ensure_initialized()?;
    let existing = db
        .get_followup(id)?
        .ok_or_else(|| MyceliumError::NotFound {
            entity: "followup".to_string(),
            id: id.to_string(),
        })?;

    // Include TZ offset so timestamps stay unambiguous across machines.
    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M %z");
    let new_body = if existing.body.trim().is_empty() {
        format!("[{}] {}", stamp, text)
    } else {
        format!("{}\n\n[{}] {}", existing.body.trim_end(), stamp, text)
    };

    let updated = db.update_followup_body(id, Some(&new_body), None, false)?;
    if !quiet {
        println!(
            "{} Appended to follow-up {} ({} chars total)",
            SUCCESS_PREFIX.green(),
            fid(updated.id),
            updated.body.len()
        );
    }
    Ok(())
}

pub fn remove(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    let fu = db
        .get_followup(id)?
        .ok_or_else(|| MyceliumError::NotFound {
            entity: "followup".to_string(),
            id: id.to_string(),
        })?;

    if !force {
        let prompt = format!("Delete follow-up {}: {}?", fid(fu.id), fu.display_title());
        if !confirm(&prompt) {
            if !quiet {
                println!("{} Cancelled", INFO_PREFIX.blue());
            }
            return Ok(());
        }
    }

    db.delete_followup(id)?;
    if !quiet {
        println!("{} Deleted follow-up {}", SUCCESS_PREFIX.green(), fid(id));
    }
    Ok(())
}

pub fn promote(id: i64, epic: Option<i64>, priority: &str, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;
    let fu = db
        .get_followup(id)?
        .ok_or_else(|| MyceliumError::NotFound {
            entity: "followup".to_string(),
            id: id.to_string(),
        })?;

    if matches!(fu.status, FollowupStatus::Done | FollowupStatus::Wontfix) {
        return Err(MyceliumError::InvalidInput(format!(
            "Follow-up {} is already {} — cannot promote",
            fid(id),
            fu.status
        )));
    }

    let priority_enum: Priority = priority.parse()?;

    let new_title = fu.display_title();
    // Always copy body into description so nothing is lost on promotion.
    // When title is missing, display_title() already returns the truncated
    // body — body becomes both title and description.
    let task = db.create_task(
        &new_title,
        Some(fu.body.as_str()),
        epic,
        priority_enum,
        None,
        None,
        None,
        Some(&format!("Promoted from follow-up {}", fid(fu.id))),
        None,
    )?;

    db.update_followup_status(
        id,
        FollowupStatus::Done,
        Some(&format!("Promoted to task {}", tid(task.id))),
    )?;

    if !quiet {
        println!(
            "{} Follow-up {} promoted to task {}: {}",
            SUCCESS_PREFIX.green(),
            fid(id),
            tid(task.id),
            task.title
        );
    }
    Ok(())
}

pub fn count(format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let counts = db.count_followups()?;

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&counts)?);
        }
        OutputFormat::Table => {
            if quiet {
                println!("{}", counts.active());
            } else {
                println!(
                    "{} Follow-ups — open: {}, in_progress: {}, done: {}, wontfix: {}",
                    INFO_PREFIX.blue(),
                    counts.open,
                    counts.in_progress,
                    counts.done,
                    counts.wontfix
                );
            }
        }
    }
    Ok(())
}

/// Silence the follow-up Stop-hook for the next `turns` stops. Writes a plain
/// counter to `.mycelium/.followup-snooze` — project-scoped by living next to
/// the DB, and read+decremented by the Stop hook. `turns == 0` clears it.
pub fn snooze(turns: u32, quiet: bool) -> Result<()> {
    // Reuse the DB dir so the snooze file is co-located with (and scoped to)
    // this project; error out the same way commands do if not initialized.
    let dir = crate::commands::get_db_path()
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or(MyceliumError::NotInitialized)?;
    if !dir.exists() {
        return Err(MyceliumError::NotInitialized);
    }
    let marker = dir.join(".followup-snooze");

    if turns == 0 {
        let _ = std::fs::remove_file(&marker);
        if !quiet {
            println!("{} Follow-up snooze cleared", SUCCESS_PREFIX.green());
        }
        return Ok(());
    }

    std::fs::write(&marker, turns.to_string())?;
    if !quiet {
        println!(
            "{} Follow-up check snoozed for the next {} stop(s)",
            SUCCESS_PREFIX.green(),
            turns
        );
    }
    Ok(())
}

/// Best-effort hint printed at end of task close. Never errors.
pub fn print_close_hint() {
    let Ok(db) = ensure_initialized() else { return };
    // Nudge only on `open` follow-ups older than the grace window: in_progress
    // items are already being worked, and a follow-up jotted down seconds ago
    // (while closing this very task) shouldn't nag about itself.
    const GRACE_SECS: i64 = 60;
    let Ok(stale_open) = db.count_stale_open_followups(GRACE_SECS) else {
        return;
    };
    if stale_open == 0 {
        return;
    }
    println!(
        "{} {} open follow-up(s) — run `myc followup list` to review",
        WARNING_PREFIX.yellow(),
        stale_open
    );
}
