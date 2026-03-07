use std::fmt;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MyceliumError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Project not initialized. Run `myc init` first")]
    NotInitialized,

    #[error("Entity not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("Task is blocked by: {0}")]
    BlockedBy(String),

    #[error("Invalid priority: {0}. Use: low, medium, high, critical")]
    InvalidPriority(String),

    #[error("Invalid status: {0}. Use: open, closed")]
    InvalidStatus(String),

    #[error("Invalid date format: {0}. Use: YYYY-MM-DD")]
    InvalidDate(String),

    #[error("Invalid GitHub reference: {0}. Use: owner/repo#number")]
    InvalidGitHubRef(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Export error: {0}")]
    Export(String),
}

pub type Result<T> = std::result::Result<T, MyceliumError>;

pub fn handle_error(err: MyceliumError) -> ! {
    eprintln!("{} {}", crate::ERROR_PREFIX, err);
    std::process::exit(1);
}

pub fn handle_usage_error(msg: &str) -> ! {
    eprintln!("{} {}", crate::ERROR_PREFIX, msg);
    std::process::exit(2);
}
