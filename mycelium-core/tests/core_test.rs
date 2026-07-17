//! Integration tests for mycelium-core against an in-memory database.
//! Covers CRUD round-trips and the serde representation of the enums (which
//! must match the on-disk DB strings, e.g. "in_progress").

use mycelium_core::models::{FollowupStatus, Priority, Status};
use mycelium_core::Database;

fn db() -> Database {
    Database::open_in_memory().expect("in-memory db")
}

#[test]
fn create_and_get_task() {
    let mut db = db();
    let epic = db.create_epic("Epic A", None, None, None).unwrap();
    let task = db
        .create_task(
            "Task 1",
            Some("desc"),
            Some(epic.id),
            Priority::High,
            None,
            None,
            Some("a,b"),
            None,
            None,
        )
        .unwrap();
    assert_eq!(task.title, "Task 1");
    assert_eq!(task.priority, Priority::High);
    assert_eq!(task.status, Status::Open);

    let fetched = db.get_task(task.id).unwrap().expect("task exists");
    assert_eq!(fetched.id, task.id);
    assert_eq!(fetched.tags.as_deref(), Some("a,b"));

    assert!(db.get_task(9999).unwrap().is_none(), "missing task -> None");
}

#[test]
fn list_tasks_filters_by_epic() {
    let mut db = db();
    let e1 = db.create_epic("E1", None, None, None).unwrap();
    let e2 = db.create_epic("E2", None, None, None).unwrap();
    db.create_task(
        "t1",
        None,
        Some(e1.id),
        Priority::Medium,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    db.create_task(
        "t2",
        None,
        Some(e2.id),
        Priority::Medium,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    let in_e1 = db
        .list_tasks(Some(e1.id), None, None, None, false, false, None)
        .unwrap();
    assert_eq!(in_e1.len(), 1);
    assert_eq!(in_e1[0].title, "t1");
}

#[test]
fn followup_lifecycle() {
    let mut db = db();
    let f = db.create_followup("something to check", None).unwrap();
    assert_eq!(f.status, FollowupStatus::Open);

    let started = db
        .update_followup_status(f.id, FollowupStatus::InProgress, None)
        .unwrap();
    assert_eq!(started.status, FollowupStatus::InProgress);

    let done = db
        .update_followup_status(f.id, FollowupStatus::Done, Some("fixed"))
        .unwrap();
    assert_eq!(done.status, FollowupStatus::Done);
    assert_eq!(done.closure_reason.as_deref(), Some("fixed"));

    let counts = db.count_followups().unwrap();
    assert_eq!(counts.done, 1);
    assert_eq!(counts.open, 0);
}

// --- serde: wire representation must equal the DB string representation ---

#[test]
fn status_serde_matches_db_string() {
    // The DB stores Display strings; the frontend receives serde strings.
    // After the snake_case alignment they must agree, especially in_progress.
    assert_eq!(
        serde_json::to_string(&Status::InProgress).unwrap(),
        "\"in_progress\""
    );
    assert_eq!(serde_json::to_string(&Status::Open).unwrap(), "\"open\"");
    assert_eq!(
        serde_json::to_string(&Status::Closed).unwrap(),
        "\"closed\""
    );
    // Display (what the DB persists) agrees.
    assert_eq!(Status::InProgress.to_string(), "in_progress");
    // Round-trip.
    let back: Status = serde_json::from_str("\"in_progress\"").unwrap();
    assert_eq!(back, Status::InProgress);
}

#[test]
fn followup_status_serde_matches_db_string() {
    assert_eq!(
        serde_json::to_string(&FollowupStatus::InProgress).unwrap(),
        "\"in_progress\""
    );
    assert_eq!(
        serde_json::to_string(&FollowupStatus::Wontfix).unwrap(),
        "\"wontfix\""
    );
    let back: FollowupStatus = serde_json::from_str("\"in_progress\"").unwrap();
    assert_eq!(back, FollowupStatus::InProgress);
}

#[test]
fn active_dependencies_exclude_closed_blockers() {
    // Regression: a task blocked only by a CLOSED task must NOT report as
    // blocked. get_active_dependencies_for_tasks filters blocked_by to
    // open/in_progress blockers; blocks (reverse edge) stays unfiltered.
    let mut db = db();
    let a = db
        .create_task(
            "A",
            None,
            None,
            Priority::Medium,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let b = db
        .create_task(
            "B",
            None,
            None,
            Priority::Medium,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    // B blocks A (A depends on B).
    db.add_dependency(a.id, b.id).unwrap();

    // While B is open, A is blocked by B.
    let deps = db.get_active_dependencies_for_tasks(&[a.id, b.id]).unwrap();
    assert_eq!(deps[&a.id].0, vec![b.id], "A blocked_by B while B open");
    assert_eq!(deps[&b.id].1, vec![a.id], "B blocks A");

    // An IN_PROGRESS blocker still counts as blocking (open+in_progress, not open-only).
    db.update_task(
        b.id,
        None,
        None,
        Some(Status::InProgress),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let deps = db.get_active_dependencies_for_tasks(&[a.id, b.id]).unwrap();
    assert_eq!(
        deps[&a.id].0,
        vec![b.id],
        "A still blocked while B in_progress"
    );

    // Close B: A must no longer be reported as blocked.
    // update_task(id, title, description, status, priority, epic_id, assignee_id,
    //             due_date, tags, notes, user_info, agent_questions)
    db.update_task(
        b.id,
        None,
        None,
        Some(Status::Closed),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let deps = db.get_active_dependencies_for_tasks(&[a.id, b.id]).unwrap();
    assert!(deps[&a.id].0.is_empty(), "A not blocked once B is closed");
    // Reverse edge still records the (now-closed) relationship.
    assert_eq!(deps[&b.id].1, vec![a.id], "blocks edge unfiltered");
}

#[test]
fn priority_serde_roundtrip() {
    for (p, s) in [
        (Priority::Low, "\"low\""),
        (Priority::Medium, "\"medium\""),
        (Priority::High, "\"high\""),
        (Priority::Critical, "\"critical\""),
    ] {
        assert_eq!(serde_json::to_string(&p).unwrap(), s);
        let back: Priority = serde_json::from_str(s).unwrap();
        assert_eq!(back, p);
    }
}
