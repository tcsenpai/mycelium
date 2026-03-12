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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Open,
    Closed,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Open => write!(f, "open"),
            Status::Closed => write!(f, "closed"),
        }
    }
}

impl std::str::FromStr for Status {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Status::Open),
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
                self.status == Status::Open && due < today
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
            Status::Closed => "✓",
        }
    }
}
