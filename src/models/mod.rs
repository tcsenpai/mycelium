use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

pub mod assignee;
pub mod dependency;
pub mod epic;
pub mod epic_note;
pub mod external_ref;
pub mod followup;
pub mod task;
pub mod task_note;

pub use assignee::Assignee;
pub use dependency::Dependency;
pub use epic::Epic;
pub use epic_note::EpicNote;
pub use external_ref::ExternalRef;
pub use followup::{Followup, FollowupStatus};
pub use task::Task;
pub use task_note::TaskNote;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Medium => write!(f, "medium"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

impl FromStr for Priority {
    type Err = crate::error::MyceliumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(Priority::Low),
            "medium" => Ok(Priority::Medium),
            "high" => Ok(Priority::High),
            "critical" => Ok(Priority::Critical),
            _ => Err(crate::error::MyceliumError::InvalidPriority(s.to_string())),
        }
    }
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Low => "low",
            Priority::Medium => "medium",
            Priority::High => "high",
            Priority::Critical => "critical",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Priority::Low => "🔵",
            Priority::Medium => "🟢",
            Priority::High => "🟠",
            Priority::Critical => "🔴",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    #[serde(rename = "in_progress")]
    InProgress,
    Closed,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Open => write!(f, "open"),
            Status::InProgress => write!(f, "in_progress"),
            Status::Closed => write!(f, "closed"),
        }
    }
}

impl FromStr for Status {
    type Err = crate::error::MyceliumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Status::Open),
            "in_progress" => Ok(Status::InProgress),
            "in-progress" => Ok(Status::InProgress),
            "closed" => Ok(Status::Closed),
            _ => Err(crate::error::MyceliumError::InvalidStatus(s.to_string())),
        }
    }
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::InProgress => "in_progress",
            Status::Closed => "closed",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Status::Open => "⭕",
            Status::InProgress => "🔄",
            Status::Closed => "✅",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExternalRefType {
    GitHubIssue,
    GitHubPr,
    Url,
}

impl fmt::Display for ExternalRefType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExternalRefType::GitHubIssue => write!(f, "github-issue"),
            ExternalRefType::GitHubPr => write!(f, "github-pr"),
            ExternalRefType::Url => write!(f, "url"),
        }
    }
}

impl std::str::FromStr for ExternalRefType {
    type Err = crate::error::MyceliumError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "github-issue" => Ok(ExternalRefType::GitHubIssue),
            "github-pr" => Ok(ExternalRefType::GitHubPr),
            "url" => Ok(ExternalRefType::Url),
            _ => Err(crate::error::MyceliumError::InvalidInput(format!(
                "Invalid ref type: {}",
                s
            ))),
        }
    }
}

impl ExternalRefType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExternalRefType::GitHubIssue => "github-issue",
            ExternalRefType::GitHubPr => "github-pr",
            ExternalRefType::Url => "url",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            ExternalRefType::GitHubIssue => "🐛",
            ExternalRefType::GitHubPr => "🔀",
            ExternalRefType::Url => "🔗",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression guard: the JSON enum string must match Display / FromStr / DB
    // ("in_progress", not "inprogress"). rename_all="lowercase" would break this.
    #[test]
    fn status_in_progress_json_uses_underscore() {
        assert_eq!(
            serde_json::to_string(&Status::InProgress).unwrap(),
            "\"in_progress\"",
        );
        assert_eq!(Status::InProgress.to_string(), "in_progress");
        assert_eq!(
            serde_json::from_str::<Status>("\"in_progress\"").unwrap(),
            Status::InProgress,
        );
    }

    #[test]
    fn followup_status_in_progress_json_uses_underscore() {
        assert_eq!(
            serde_json::to_string(&FollowupStatus::InProgress).unwrap(),
            "\"in_progress\"",
        );
        assert_eq!(FollowupStatus::InProgress.to_string(), "in_progress");
    }
}
