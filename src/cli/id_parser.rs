//! clap `value_parser` adapters for category-prefixed IDs (v5).
//!
//! Each function parses a user-supplied ID token (bare `3` or prefixed
//! `T3`/`E3`/`F3`, case-insensitive) into the bare `i64` the command
//! handlers already expect. A mismatched prefix is a hard error — see
//! `mycelium_core::id::parse_id`.

use mycelium_core::id::{parse_id, IdKind};

pub fn epic_id(s: &str) -> Result<i64, String> {
    parse_id(IdKind::Epic, s)
}

pub fn task_id(s: &str) -> Result<i64, String> {
    parse_id(IdKind::Task, s)
}

pub fn followup_id(s: &str) -> Result<i64, String> {
    parse_id(IdKind::Followup, s)
}

pub fn assignee_id(s: &str) -> Result<i64, String> {
    parse_id(IdKind::Assignee, s)
}

pub fn ref_id(s: &str) -> Result<i64, String> {
    parse_id(IdKind::Ref, s)
}
