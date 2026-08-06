//! Category-prefixed entity IDs (v5).
//!
//! Each entity has its own integer sequence, so a bare `3` is ambiguous
//! between an epic, a task, and a follow-up. v5 gives every category a
//! single-letter display prefix — `E3`, `T3`, `F3` — while parsing stays
//! backward compatible: a bare integer is always accepted.

/// A prefixable entity category. The `char` is the display prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdKind {
    Epic,
    Task,
    Followup,
    Assignee,
    /// External reference (`myc task unlink <ref_id>`).
    Ref,
}

impl IdKind {
    /// Uppercase single-letter display prefix.
    pub fn prefix(self) -> char {
        match self {
            IdKind::Epic => 'E',
            IdKind::Task => 'T',
            IdKind::Followup => 'F',
            IdKind::Assignee => 'A',
            IdKind::Ref => 'R',
        }
    }

    /// Human-readable name for error messages.
    pub fn name(self) -> &'static str {
        match self {
            IdKind::Epic => "Epic",
            IdKind::Task => "Task",
            IdKind::Followup => "Follow-up",
            IdKind::Assignee => "Assignee",
            IdKind::Ref => "External reference",
        }
    }

    /// The `myc` noun used in "did you mean" hints (`epic`, `task`, …).
    pub fn command_noun(self) -> &'static str {
        match self {
            IdKind::Epic => "epic",
            IdKind::Task => "task",
            IdKind::Followup => "followup",
            IdKind::Assignee => "assignee",
            IdKind::Ref => "task",
        }
    }

    fn from_prefix(c: char) -> Option<IdKind> {
        match c.to_ascii_uppercase() {
            'E' => Some(IdKind::Epic),
            'T' => Some(IdKind::Task),
            'F' => Some(IdKind::Followup),
            'A' => Some(IdKind::Assignee),
            'R' => Some(IdKind::Ref),
            _ => None,
        }
    }
}

/// Format an ID for display, e.g. `format_id(IdKind::Task, 3)` → `"T3"`.
pub fn format_id(kind: IdKind, id: i64) -> String {
    format!("{}{}", kind.prefix(), id)
}

/// Parse a user-supplied ID token for a command that expects `kind`.
///
/// Accepts:
/// - a bare integer (`3`) — backward compatible,
/// - the matching prefix (`T3`, case-insensitive) → the bare id,
/// - errors on a mismatched prefix (`E3` for a task command) with a hint.
pub fn parse_id(kind: IdKind, s: &str) -> Result<i64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty ID".to_string());
    }

    let first = s.chars().next().unwrap();
    if first.is_ascii_digit() || first == '-' {
        // Bare integer path (backward compatible).
        return s
            .parse::<i64>()
            .map_err(|_| format!("'{}' is not a valid ID", s));
    }

    // Prefixed path: <letter><digits>.
    let parsed_kind = IdKind::from_prefix(first)
        .ok_or_else(|| format!("'{}' is not a valid ID (unknown prefix '{}')", s, first))?;
    let digits = &s[first.len_utf8()..];
    let num = digits
        .parse::<i64>()
        .map_err(|_| format!("'{}' is not a valid ID", s))?;

    if parsed_kind == kind {
        Ok(num)
    } else {
        Err(format!(
            "'{}' is {} ({}{}), but this command expects {}. \
             Did you mean a `myc {}` command?",
            s,
            parsed_kind.name(),
            parsed_kind.prefix(),
            num,
            kind.name(),
            parsed_kind.command_noun(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_integer_backward_compat() {
        assert_eq!(parse_id(IdKind::Task, "3").unwrap(), 3);
        assert_eq!(parse_id(IdKind::Epic, "0").unwrap(), 0);
    }

    #[test]
    fn matching_prefix_case_insensitive() {
        assert_eq!(parse_id(IdKind::Task, "T3").unwrap(), 3);
        assert_eq!(parse_id(IdKind::Task, "t3").unwrap(), 3);
        assert_eq!(parse_id(IdKind::Epic, "E1").unwrap(), 1);
        assert_eq!(parse_id(IdKind::Followup, "f7").unwrap(), 7);
    }

    #[test]
    fn mismatched_prefix_errors() {
        let err = parse_id(IdKind::Task, "E3").unwrap_err();
        assert!(err.contains("Epic"));
        assert!(err.contains("myc epic"));
    }

    #[test]
    fn garbage_errors() {
        assert!(parse_id(IdKind::Task, "abc").is_err());
        assert!(parse_id(IdKind::Task, "T").is_err());
        assert!(parse_id(IdKind::Task, "").is_err());
        assert!(parse_id(IdKind::Task, "3x").is_err());
    }

    #[test]
    fn format_roundtrip() {
        assert_eq!(format_id(IdKind::Task, 3), "T3");
        assert_eq!(format_id(IdKind::Epic, 1), "E1");
        assert_eq!(format_id(IdKind::Followup, 7), "F7");
        assert_eq!(
            parse_id(IdKind::Task, &format_id(IdKind::Task, 42)).unwrap(),
            42
        );
    }
}
