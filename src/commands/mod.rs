use colored::Colorize;
use crate::db::Database;
use crate::error::Result;
use crate::cli::OutputFormat;

pub mod epic;
pub mod task;
pub mod assignee;
pub mod deps;
pub mod init;
pub mod list;
pub mod summary;
pub mod export;
pub mod doctor;

pub const ERROR_PREFIX: &str = "❌";
pub const SUCCESS_PREFIX: &str = "✅";
pub const INFO_PREFIX: &str = "ℹ️";
pub const WARNING_PREFIX: &str = "⚠️";

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

pub fn format_output<T: serde::Serialize>(data: &T, format: &OutputFormat, quiet: bool) -> Result<()> {
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
