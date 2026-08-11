use crate::cli::OutputFormat;
use crate::db::Database;
use crate::error::Result;
use colored::Colorize;

pub mod assignee;
pub mod deps;
pub mod doctor;
pub mod epic;
pub mod export;
pub mod followup;
pub mod hooks;
pub mod init;
pub mod linear;
pub mod list;
pub mod summary;
pub mod task;

pub const ERROR_PREFIX: &str = "❌";
pub const SUCCESS_PREFIX: &str = "✅";
pub const INFO_PREFIX: &str = "ℹ️";
pub const WARNING_PREFIX: &str = "⚠️";

// Category-prefixed ID display helpers (v5): T3 / E3 / F3 / A3 / R3.
// Human-facing output only — bare-echo and JSON paths keep raw integers.
use mycelium_core::id::{format_id, IdKind};

pub fn tid(id: i64) -> String {
    format_id(IdKind::Task, id)
}
pub fn eid(id: i64) -> String {
    format_id(IdKind::Epic, id)
}
pub fn fid(id: i64) -> String {
    format_id(IdKind::Followup, id)
}
pub fn aid(id: i64) -> String {
    format_id(IdKind::Assignee, id)
}
pub fn rid(id: i64) -> String {
    format_id(IdKind::Ref, id)
}

pub fn get_db_path() -> std::path::PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".mycelium")
        .join("mycelium.db")
}

pub fn ensure_initialized() -> Result<Database> {
    let db_path = get_db_path();
    if !db_path.exists() {
        return Err(crate::error::MyceliumError::NotInitialized);
    }
    Database::open(db_path)
}

pub fn format_output<T: serde::Serialize>(
    data: &T,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    if quiet {
        return Ok(());
    }

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
        OutputFormat::Table => {
            // Table formatting is handled per-command
        }
    }
    Ok(())
}

pub fn confirm(prompt: &str) -> bool {
    print!("{} {} [y/N] ", WARNING_PREFIX.yellow(), prompt);
    use std::io::Write;
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or(0);

    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}
