use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Epic {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: super::Status,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

impl Epic {
    pub fn new(id: i64, title: impl Into<String>) -> Self {
        let now = Local::now();
        Self {
            id,
            title: title.into(),
            description: None,
            status: super::Status::Open,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn close(&mut self) {
        self.status = super::Status::Closed;
        self.updated_at = Local::now();
    }

    pub fn reopen(&mut self) {
        self.status = super::Status::Open;
        self.updated_at = Local::now();
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EpicSummary {
    #[serde(flatten)]
    pub epic: Epic,
    pub total_tasks: i64,
    pub open_tasks: i64,
    pub closed_tasks: i64,
}
