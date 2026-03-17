use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNote {
    pub id: i64,
    pub task_id: i64,
    pub content: String,
    pub created_at: DateTime<Local>,
}

impl TaskNote {
    pub fn new(id: i64, task_id: i64, content: impl Into<String>) -> Self {
        Self {
            id,
            task_id,
            content: content.into(),
            created_at: Local::now(),
        }
    }
}
