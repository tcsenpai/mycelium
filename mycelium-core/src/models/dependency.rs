use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub id: i64,
    pub task_id: i64,
    pub depends_on_task_id: i64,
    pub created_at: DateTime<Local>,
}

impl Dependency {
    pub fn new(id: i64, task_id: i64, depends_on_task_id: i64) -> Self {
        Self {
            id,
            task_id,
            depends_on_task_id,
            created_at: Local::now(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DependencyChain {
    pub task_id: i64,
    pub blocked_by: Vec<i64>,
    pub blocks: Vec<i64>,
    pub all_dependencies: Vec<i64>,
}
