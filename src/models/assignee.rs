use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignee {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub github_username: Option<String>,
    pub created_at: DateTime<Local>,
}

impl Assignee {
    pub fn new(id: i64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            email: None,
            github_username: None,
            created_at: Local::now(),
        }
    }

    pub fn with_email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    pub fn with_github(mut self, github: impl Into<String>) -> Self {
        self.github_username = Some(github.into());
        self
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AssigneeWithStats {
    #[serde(flatten)]
    pub assignee: Assignee,
    pub total_tasks: i64,
    pub open_tasks: i64,
}
