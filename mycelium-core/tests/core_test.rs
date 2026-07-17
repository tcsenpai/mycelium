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
