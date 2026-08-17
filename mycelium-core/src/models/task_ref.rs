use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::MyceliumError;

/// A non-blocking, symmetric relationship between two tasks. Unlike a
/// dependency (directional, gates state transitions via the blocker guard), a
/// `TaskRef` only annotates: "these two are related" or "these two are the same
/// thing". It never blocks anything and never closes/merges anything on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskRefType {
    /// The two tasks are related (same family/area), neither blocks the other.
    Relates,
    /// The two tasks are duplicates. Marking only — the decision to close one
    /// stays with the user.
    Duplicate,
}

impl fmt::Display for TaskRefType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for TaskRefType {
    type Err = MyceliumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "relates" | "relates-to" | "related" => Ok(TaskRefType::Relates),
            "duplicate" | "duplicate-of" | "dup" => Ok(TaskRefType::Duplicate),
            _ => Err(MyceliumError::InvalidInput(format!(
                "Invalid task ref type: {s} (expected 'relates' or 'duplicate')"
            ))),
        }
    }
}

impl TaskRefType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskRefType::Relates => "relates",
            TaskRefType::Duplicate => "duplicate",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            TaskRefType::Relates => "🔗",
            TaskRefType::Duplicate => "👯",
        }
    }
}

/// A stored task reference row. One row per link (the relation is symmetric, so
/// reads match both directions rather than storing a mirror row).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRef {
    pub id: i64,
    pub task_id: i64,
    pub related_task_id: i64,
    pub ref_type: TaskRefType,
    pub created_at: DateTime<Local>,
}

/// A task reference as seen FROM a given task's perspective, with the other
/// task resolved to its title. This is what `myc task refs <ID>` returns and
/// what the JSON shape `{id, ref_type, other_id, title}` serializes from.
#[derive(Debug, Clone, Serialize)]
pub struct TaskRefView {
    /// The task_refs row id (usable for unlink).
    pub id: i64,
    pub ref_type: TaskRefType,
    /// The OTHER task in the relation (never the task you queried).
    pub other_id: i64,
    /// The other task's title, resolved for display.
    pub title: String,
}
