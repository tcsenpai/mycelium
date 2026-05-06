use sha2::{Sha256, Digest};

use crate::error::{MyceliumError, Result};
use crate::models::{Priority, Status, Task};
use super::client::{LinearIssue, LinearWorkflowState};
use super::config::LinearConfig;

/// Compute a hash of the key fields of a task for change detection.
pub fn hash_task(task: &Task) -> String {
    let mut hasher = Sha256::new();
    hasher.update(task.title.as_bytes());
    hasher.update(task.description.as_deref().unwrap_or("").as_bytes());
    hasher.update(task.status.to_string().as_bytes());
    hasher.update(task.priority.to_string().as_bytes());
    hasher.update(
        task.assignee_id
            .map(|id| id.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(
        task.due_date
            .map(|d| d.to_string())
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(task.tags.as_deref().unwrap_or("").as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Compute a hash of the key fields of a Linear issue for change detection.
pub fn hash_issue(issue: &LinearIssue) -> String {
    let mut hasher = Sha256::new();
    hasher.update(issue.title.as_bytes());
    hasher.update(issue.description.as_deref().unwrap_or("").as_bytes());
    hasher.update(issue.state.name.as_bytes());
    hasher.update(issue.priority.to_string().as_bytes());
    hasher.update(
        issue
            .assignee
            .as_ref()
            .map(|a| a.id.as_str())
            .unwrap_or("")
            .as_bytes(),
    );
    hasher.update(issue.due_date.as_deref().unwrap_or("").as_bytes());
    let label_names: Vec<&str> = issue.labels.nodes.iter().map(|l| l.name.as_str()).collect();
    hasher.update(label_names.join(",").as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Map local priority to Linear priority number.
pub fn priority_to_linear(priority: &Priority, config: &LinearConfig) -> u8 {
    let key = priority.to_string();
    config
        .mapping
        .priority
        .get(&key)
        .copied()
        .unwrap_or(match priority {
            Priority::Critical => 1,
            Priority::High => 2,
            Priority::Medium => 3,
            Priority::Low => 4,
        })
}

/// Map Linear priority number to local priority.
pub fn priority_from_linear(linear_priority: u8) -> Priority {
    match linear_priority {
        1 => Priority::Critical,
        2 => Priority::High,
        3 => Priority::Medium,
        4 => Priority::Low,
        _ => Priority::Medium,
    }
}

/// Find the Linear workflow state ID for a local status.
pub fn status_to_linear_state_id(
    status: &Status,
    states: &[LinearWorkflowState],
    config: &LinearConfig,
) -> Result<String> {
    let target_name = match status {
        Status::Open => config
            .mapping
            .status
            .get("open")
            .cloned()
            .unwrap_or_else(|| "Todo".to_string()),
        Status::InProgress => config
            .mapping
            .status
            .get("in_progress")
            .cloned()
            .unwrap_or_else(|| "In Progress".to_string()),
        Status::Closed => config
            .mapping
            .status
            .get("closed")
            .cloned()
            .unwrap_or_else(|| "Done".to_string()),
    };

    // Try exact name match first
    if let Some(state) = states.iter().find(|s| s.name.eq_ignore_ascii_case(&target_name)) {
        return Ok(state.id.clone());
    }

    // Fallback: match by state type
    let fallback_type = match status {
        Status::Open => "unstarted",
        Status::InProgress => "started",
        Status::Closed => "completed",
    };
    states
        .iter()
        .find(|s| s.state_type == fallback_type)
        .map(|s| s.id.clone())
        .ok_or_else(|| {
            MyceliumError::LinearApi(format!(
                "No workflow state found for status '{}' (tried '{}' and type '{}')",
                status, target_name, fallback_type
            ))
        })
}

/// Map a Linear workflow state to local status.
pub fn status_from_linear(state: &LinearWorkflowState) -> Status {
    match state.state_type.as_str() {
        "completed" | "cancelled" => Status::Closed,
        "started" | "in_progress" => Status::InProgress,
        _ => Status::Open,
    }
}

/// Try to match a local assignee to a Linear user by email or name.
pub fn find_linear_user_id(
    assignee_email: Option<&str>,
    assignee_name: Option<&str>,
    assignee_github: Option<&str>,
    config: &LinearConfig,
    team_members: &[super::client::LinearUser],
) -> Option<String> {
    // Check explicit mapping first
    if let Some(email) = assignee_email {
        if let Some(id) = config.mapping.assignees.get(email) {
            return Some(id.clone());
        }
    }
    if let Some(name) = assignee_name {
        if let Some(id) = config.mapping.assignees.get(name) {
            return Some(id.clone());
        }
    }

    // Try matching by email
    if let Some(email) = assignee_email {
        if let Some(user) = team_members.iter().find(|u| u.email.eq_ignore_ascii_case(email)) {
            return Some(user.id.clone());
        }
    }

    // Try matching by name
    if let Some(name) = assignee_name {
        if let Some(user) = team_members.iter().find(|u| {
            u.name.eq_ignore_ascii_case(name) || u.display_name.eq_ignore_ascii_case(name)
        }) {
            return Some(user.id.clone());
        }
    }

    // Try git username match against display name
    if let Some(github) = assignee_github {
        if let Some(user) = team_members
            .iter()
            .find(|u| u.display_name.eq_ignore_ascii_case(github) || u.name.eq_ignore_ascii_case(github))
        {
            return Some(user.id.clone());
        }
    }

    None
}
