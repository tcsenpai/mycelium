use colored::Colorize;
use comfy_table::{Table, ContentArrangement};
use crate::commands::{ensure_initialized, INFO_PREFIX};
use crate::cli::OutputFormat;
use crate::models::{Priority, Status};
use crate::error::Result;

pub fn execute(
    epic_id: Option<i64>,
    status: Option<&str>,
    priority: Option<&str>,
    assignee_id: Option<i64>,
    blocked: bool,
    overdue: bool,
    format: &OutputFormat,
    quiet: bool,
) -> Result<()> {
    // This is just a convenience wrapper around task list
    super::task::list(epic_id, status, priority, assignee_id, blocked, overdue, format, quiet)
}
