use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpicNote {
    pub id: i64,
    pub epic_id: i64,
    pub content: String,
    pub created_at: DateTime<Local>,
}
