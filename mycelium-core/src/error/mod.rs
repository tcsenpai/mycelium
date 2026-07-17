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

    #[error("Invalid status: {0}. Use: open, in_progress, closed")]
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

    #[error("Linear API error: {0}")]
    LinearApi(String),

    #[error("Linear config error: {0}")]
    LinearConfig(String),

    #[error("Linear sync conflict on task {task_id}: {message}")]
    LinearSyncConflict { task_id: i64, message: String },

    #[error("HTTP error: {0}")]
    Http(String),
}

pub type Result<T> = std::result::Result<T, MyceliumError>;

/// Serialize errors as `{ "error": "<display message>" }` so GUI consumers
/// (Tauri commands) can return them to a frontend. The rich variants collapse
/// to their Display string, which is all a frontend needs.
impl serde::Serialize for MyceliumError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("MyceliumError", 1)?;
        s.serialize_field("error", &self.to_string())?;
        s.end()
    }
}
