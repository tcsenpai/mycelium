use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowupStatus {
    Open,
    InProgress,
    Done,
    Wontfix,
}

impl fmt::Display for FollowupStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FollowupStatus {
    type Err = crate::error::MyceliumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(FollowupStatus::Open),
            "in_progress" | "in-progress" => Ok(FollowupStatus::InProgress),
            "done" => Ok(FollowupStatus::Done),
            "wontfix" | "won't-fix" | "wont-fix" => Ok(FollowupStatus::Wontfix),
            _ => Err(crate::error::MyceliumError::InvalidInput(format!(
                "Invalid follow-up status: {}. Use: open, in_progress, done, wontfix",
                s
            ))),
        }
    }
}

impl FollowupStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FollowupStatus::Open => "open",
            FollowupStatus::InProgress => "in_progress",
            FollowupStatus::Done => "done",
            FollowupStatus::Wontfix => "wontfix",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            FollowupStatus::Open => "📌",
            FollowupStatus::InProgress => "🔄",
            FollowupStatus::Done => "✅",
            FollowupStatus::Wontfix => "🚫",
        }
    }

    pub fn is_open(&self) -> bool {
        matches!(self, FollowupStatus::Open | FollowupStatus::InProgress)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Followup {
    pub id: i64,
    pub body: String,
    pub title: Option<String>,
    pub status: FollowupStatus,
    pub closure_reason: Option<String>,
    pub created_at: DateTime<Local>,
    pub closed_at: Option<DateTime<Local>>,
}

impl Followup {
    pub fn display_title(&self) -> String {
        if let Some(t) = &self.title {
            if !t.is_empty() {
                return t.clone();
            }
        }
        let trimmed = self.body.trim();
        if trimmed.chars().count() <= 60 {
            trimmed.to_string()
        } else {
            let truncated: String = trimmed.chars().take(60).collect();
            format!("{}…", truncated)
        }
    }
}
