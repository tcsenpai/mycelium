use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use super::{Priority, Status};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: Status,
    pub priority: Priority,
    pub epic_id: Option<i64>,
    pub assignee_id: Option<i64>,
    pub due_date: Option<NaiveDate>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

impl Task {
    pub fn new(id: i64, title: impl Into<String>) -> Self {
        let now = Local::now();
        Self {
            id,
            title: title.into(),
            description: None,
            status: Status::Open,
            priority: Priority::Medium,
            epic_id: None,
            assignee_id: None,
            due_date: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_epic(mut self, epic_id: i64) -> Self {
        self.epic_id = Some(epic_id);
        self
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_assignee(mut self, assignee_id: i64) -> Self {
        self.assignee_id = Some(assignee_id);
        self
    }

    pub fn with_due_date(mut self, due_date: NaiveDate) -> Self {
        self.due_date = Some(due_date);
        self
    }

    pub fn close(&mut self) {
        self.status = Status::Closed;
        self.updated_at = Local::now();
    }

    pub fn reopen(&mut self) {
        self.status = Status::Open;
        self.updated_at = Local::now();
    }

    pub fn is_overdue(&self) -> bool {
        match self.due_date {
            Some(due) => {
                let today = Local::now().naive_local().date();
                self.status == Status::Open && due < today
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskWithRelations {
    pub task: Task,
    pub epic_title: Option<String>,
    pub assignee_name: Option<String>,
    pub blocked_by: Vec<i64>,
    pub blocks: Vec<i64>,
    pub external_refs: Vec<super::ExternalRef>,
}
