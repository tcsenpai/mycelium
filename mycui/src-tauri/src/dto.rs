// Frontend-facing DTOs (Data Transfer Objects).
//
// These types are the wire contract with the React frontend (mycui/src/lib/types.ts).
// They are copied verbatim from the previous mycui/src-tauri/src/models.rs so that
// the JSON shape sent over Tauri's IPC bridge is byte-for-byte unchanged, even
// though the underlying storage/query logic now lives in `mycelium_core`.
//
// Do NOT rename fields or change serde attributes here without also updating
// mycui/src/lib/types.ts.

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: Status,
    pub priority: Priority,
    pub epic_id: Option<i64>,
    pub epic_title: Option<String>,
    pub assignee_id: Option<i64>,
    pub assignee_name: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub tags: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub blocked_by: Vec<i64>,
    pub blocks: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
    pub epic_id: Option<i64>,
    pub priority: Priority,
    pub assignee_id: Option<i64>,
    pub due_date: Option<String>, // YYYY-MM-DD
    pub tags: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub epic_id: Option<Option<i64>>,
    pub assignee_id: Option<Option<i64>>,
    pub due_date: Option<Option<String>>,
    pub tags: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Epic {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: Status,
    pub total_tasks: i64,
    pub open_tasks: i64,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEpic {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpicUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<Status>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignee {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub github_username: Option<String>,
    pub total_tasks: i64,
    pub open_tasks: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAssignee {
    pub name: String,
    pub email: Option<String>,
    pub github_username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyChain {
    pub task_id: i64,
    pub blocked_by: Vec<i64>,
    pub blocks: Vec<i64>,
    pub all_dependencies: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_epics: i64,
    pub open_epics: i64,
    pub closed_epics: i64,
    pub total_tasks: i64,
    pub open_tasks: i64,
    pub closed_tasks: i64,
    pub overdue_tasks: i64,
    pub blocked_tasks: i64,
    pub high_priority_open: i64,
    pub completion_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TaskFilters {
    pub epic_id: Option<i64>,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub assignee_id: Option<i64>,
    pub tag: Option<String>,
    pub blocked: bool,
    pub overdue: bool,
    pub search: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFollowup {
    pub body: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FollowupCounts {
    pub open: i64,
    pub in_progress: i64,
    pub done: i64,
    pub wontfix: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FollowupStatus {
    Open,
    InProgress,
    Done,
    Wontfix,
}

impl std::fmt::Display for FollowupStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FollowupStatus::Open => "open",
            FollowupStatus::InProgress => "in_progress",
            FollowupStatus::Done => "done",
            FollowupStatus::Wontfix => "wontfix",
        })
    }
}

impl std::str::FromStr for FollowupStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(FollowupStatus::Open),
            "in_progress" | "in-progress" => Ok(FollowupStatus::InProgress),
            "done" => Ok(FollowupStatus::Done),
            "wontfix" | "won't-fix" | "wont-fix" => Ok(FollowupStatus::Wontfix),
            _ => Err(format!("Invalid followup status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    InProgress,
    Closed,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Open => write!(f, "open"),
            Status::InProgress => write!(f, "in_progress"),
            Status::Closed => write!(f, "closed"),
        }
    }
}

impl std::str::FromStr for Status {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Status::Open),
            "in_progress" | "in-progress" => Ok(Status::InProgress),
            "closed" => Ok(Status::Closed),
            _ => Err(format!("Invalid status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Medium => write!(f, "medium"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for Priority {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(Priority::Low),
            "medium" => Ok(Priority::Medium),
            "high" => Ok(Priority::High),
            "critical" => Ok(Priority::Critical),
            _ => Err(format!("Invalid priority: {}", s)),
        }
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Medium
    }
}

impl Task {
    pub fn is_overdue(&self) -> bool {
        match self.due_date {
            Some(due) => {
                let today = Local::now().naive_local().date();
                (self.status == Status::Open || self.status == Status::InProgress) && due < today
            }
            None => false,
        }
    }

    pub fn priority_color(&self) -> &'static str {
        match self.priority {
            Priority::Low => "#3b82f6",      // blue-500
            Priority::Medium => "#22c55e",   // green-500
            Priority::High => "#f97316",     // orange-500
            Priority::Critical => "#ef4444", // red-500
        }
    }

    pub fn status_icon(&self) -> &'static str {
        match self.status {
            Status::Open => "○",
            Status::InProgress => "◐",
            Status::Closed => "✓",
        }
    }
}

// ---------------------------------------------------------------------------
// Conversions: mycelium_core types -> DTOs
// ---------------------------------------------------------------------------

impl From<mycelium_core::models::Status> for Status {
    fn from(s: mycelium_core::models::Status) -> Self {
        match s {
            mycelium_core::models::Status::Open => Status::Open,
            mycelium_core::models::Status::InProgress => Status::InProgress,
            mycelium_core::models::Status::Closed => Status::Closed,
        }
    }
}

impl From<Status> for mycelium_core::models::Status {
    fn from(s: Status) -> Self {
        match s {
            Status::Open => mycelium_core::models::Status::Open,
            Status::InProgress => mycelium_core::models::Status::InProgress,
            Status::Closed => mycelium_core::models::Status::Closed,
        }
    }
}

impl From<mycelium_core::models::Priority> for Priority {
    fn from(p: mycelium_core::models::Priority) -> Self {
        match p {
            mycelium_core::models::Priority::Low => Priority::Low,
            mycelium_core::models::Priority::Medium => Priority::Medium,
            mycelium_core::models::Priority::High => Priority::High,
            mycelium_core::models::Priority::Critical => Priority::Critical,
        }
    }
}

impl From<Priority> for mycelium_core::models::Priority {
    fn from(p: Priority) -> Self {
        match p {
            Priority::Low => mycelium_core::models::Priority::Low,
            Priority::Medium => mycelium_core::models::Priority::Medium,
            Priority::High => mycelium_core::models::Priority::High,
            Priority::Critical => mycelium_core::models::Priority::Critical,
        }
    }
}

impl From<mycelium_core::models::FollowupStatus> for FollowupStatus {
    fn from(s: mycelium_core::models::FollowupStatus) -> Self {
        match s {
            mycelium_core::models::FollowupStatus::Open => FollowupStatus::Open,
            mycelium_core::models::FollowupStatus::InProgress => FollowupStatus::InProgress,
            mycelium_core::models::FollowupStatus::Done => FollowupStatus::Done,
            mycelium_core::models::FollowupStatus::Wontfix => FollowupStatus::Wontfix,
        }
    }
}

impl From<FollowupStatus> for mycelium_core::models::FollowupStatus {
    fn from(s: FollowupStatus) -> Self {
        match s {
            FollowupStatus::Open => mycelium_core::models::FollowupStatus::Open,
            FollowupStatus::InProgress => mycelium_core::models::FollowupStatus::InProgress,
            FollowupStatus::Done => mycelium_core::models::FollowupStatus::Done,
            FollowupStatus::Wontfix => mycelium_core::models::FollowupStatus::Wontfix,
        }
    }
}

impl From<mycelium_core::models::Followup> for Followup {
    fn from(f: mycelium_core::models::Followup) -> Self {
        Followup {
            id: f.id,
            body: f.body,
            title: f.title,
            status: f.status.into(),
            closure_reason: f.closure_reason,
            created_at: f.created_at,
            closed_at: f.closed_at,
        }
    }
}

impl From<mycelium_core::db::FollowupCounts> for FollowupCounts {
    fn from(c: mycelium_core::db::FollowupCounts) -> Self {
        FollowupCounts {
            open: c.open,
            in_progress: c.in_progress,
            done: c.done,
            wontfix: c.wontfix,
        }
    }
}

impl From<mycelium_core::db::DashboardStats> for DashboardStats {
    fn from(s: mycelium_core::db::DashboardStats) -> Self {
        DashboardStats {
            total_epics: s.total_epics,
            open_epics: s.open_epics,
            closed_epics: s.closed_epics,
            total_tasks: s.total_tasks,
            open_tasks: s.open_tasks,
            closed_tasks: s.closed_tasks,
            overdue_tasks: s.overdue_tasks,
            blocked_tasks: s.blocked_tasks,
            high_priority_open: s.high_priority_open,
            completion_rate: s.completion_rate,
        }
    }
}

impl From<mycelium_core::models::dependency::DependencyChain> for DependencyChain {
    fn from(d: mycelium_core::models::dependency::DependencyChain) -> Self {
        DependencyChain {
            task_id: d.task_id,
            blocked_by: d.blocked_by,
            blocks: d.blocks,
            all_dependencies: d.all_dependencies,
        }
    }
}

/// Convert a core `Task` plus looked-up relations into the frontend `Task` DTO.
/// `epic_title`/`assignee_name` are resolved by the caller (batched lookups)
/// to avoid N+1 queries; `blocked_by`/`blocks` come from
/// `get_dependencies_for_tasks`.
pub fn task_from_core(
    t: mycelium_core::models::Task,
    epic_title: Option<String>,
    assignee_name: Option<String>,
    blocked_by: Vec<i64>,
    blocks: Vec<i64>,
) -> Task {
    Task {
        id: t.id,
        title: t.title,
        description: t.description,
        status: t.status.into(),
        priority: t.priority.into(),
        epic_id: t.epic_id,
        epic_title,
        assignee_id: t.assignee_id,
        assignee_name,
        due_date: t.due_date,
        tags: t.tags,
        created_at: t.created_at,
        updated_at: t.updated_at,
        blocked_by,
        blocks,
    }
}

impl From<mycelium_core::models::epic::EpicSummary> for Epic {
    fn from(s: mycelium_core::models::epic::EpicSummary) -> Self {
        Epic {
            id: s.epic.id,
            title: s.epic.title,
            description: s.epic.description,
            status: s.epic.status.into(),
            total_tasks: s.total_tasks,
            open_tasks: s.open_tasks,
            created_at: s.epic.created_at,
            updated_at: s.epic.updated_at,
        }
    }
}

impl From<mycelium_core::models::assignee::AssigneeWithStats> for Assignee {
    fn from(s: mycelium_core::models::assignee::AssigneeWithStats) -> Self {
        Assignee {
            id: s.assignee.id,
            name: s.assignee.name,
            email: s.assignee.email,
            github_username: s.assignee.github_username,
            total_tasks: s.total_tasks,
            open_tasks: s.open_tasks,
        }
    }
}
