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
fn fresh_db_reports_latest_schema_version() {
    // Regression for a doctor bug that hard-coded "v2": a freshly-migrated DB
    // must report the current schema version, matching LATEST_SCHEMA_VERSION.
    let db = db();
    assert_eq!(
        db.schema_version().unwrap(),
        mycelium_core::db::LATEST_SCHEMA_VERSION
    );
    assert!(mycelium_core::db::LATEST_SCHEMA_VERSION >= 6);
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

#[test]
fn batch_close_reports_every_outcome_bucket() {
    // Regression for a doctor/batch-close bug: in_progress tasks never
    // closed (UPDATE only matched status='open'), already-closed tasks were
    // silently re-counted as newly closed, and blocked-vs-missing were
    // conflated. Exercise one task in each state and assert each lands in
    // the right outcome bucket.
    let mut db = db();

    let open_task = db
        .create_task(
            "Open task",
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
    let in_progress_task = db
        .create_task(
            "In progress task",
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
    db.update_task(
        in_progress_task.id,
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

    let already_closed_task = db
        .create_task(
            "Already closed",
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
    db.update_task(
        already_closed_task.id,
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

    let blocked_task = db
        .create_task(
            "Blocked task",
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
    let blocker_task = db
        .create_task(
            "Blocker (stays open)",
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
    db.add_dependency(blocked_task.id, blocker_task.id).unwrap();

    let nonexistent_id = 999_999;

    let ids = [
        open_task.id,
        in_progress_task.id,
        already_closed_task.id,
        blocked_task.id,
        nonexistent_id,
    ];

    let outcome = db.batch_close_tasks(&ids, false).unwrap();

    let closed_ids: Vec<i64> = outcome.closed.iter().map(|t| t.id).collect();
    assert!(closed_ids.contains(&open_task.id), "open task should close");
    assert!(
        closed_ids.contains(&in_progress_task.id),
        "in_progress task should close too (was previously skipped)"
    );
    assert_eq!(outcome.closed.len(), 2);

    assert_eq!(outcome.already_closed, vec![already_closed_task.id]);
    assert_eq!(outcome.blocked, vec![blocked_task.id]);
    assert_eq!(outcome.not_found, vec![nonexistent_id]);

    // The blocker itself and the already-closed task are untouched.
    assert_eq!(
        db.get_task(blocked_task.id).unwrap().unwrap().status,
        Status::Open
    );
    assert_eq!(
        db.get_task(already_closed_task.id).unwrap().unwrap().status,
        Status::Closed
    );

    // force=true should now close the previously-blocked task.
    let forced = db.batch_close_tasks(&[blocked_task.id], true).unwrap();
    assert_eq!(forced.closed.len(), 1);
    assert_eq!(forced.closed[0].id, blocked_task.id);
}

#[test]
fn batch_add_tag_dedupes_exact_tokens_not_substrings() {
    // Regression: `current_tags.contains(tag)` matched "ui" inside "build",
    // silently refusing to add a distinct tag. Comparison must be over exact
    // comma-separated tokens.
    let mut db = db();
    let task = db
        .create_task(
            "Task",
            None,
            None,
            Priority::Medium,
            None,
            None,
            Some("build"),
            None,
            None,
        )
        .unwrap();
    let missing_id = 424242;

    let (updated, not_found) = db.batch_add_tag(&[task.id, missing_id], "ui").unwrap();

    assert_eq!(updated.len(), 1);
    let tags = updated[0].tags.clone().unwrap();
    let tokens: Vec<&str> = tags.split(',').map(|t| t.trim()).collect();
    assert!(
        tokens.contains(&"ui"),
        "expected 'ui' tag to be added: {tags}"
    );
    assert!(tokens.contains(&"build"));
    assert_eq!(not_found, vec![missing_id]);

    // Adding the same tag again must not duplicate it.
    let (updated_again, _) = db.batch_add_tag(&[task.id], "ui").unwrap();
    let tags_again = updated_again[0].tags.clone().unwrap();
    let count = tags_again.split(',').filter(|t| t.trim() == "ui").count();
    assert_eq!(count, 1, "tag must not be duplicated: {tags_again}");
}

#[test]
fn batch_close_dedupes_repeated_ids() {
    // A repeated id must be acted on and reported exactly once, not
    // mis-bucketed into not_found on its second occurrence.
    let mut db = db();
    let t = db
        .create_task(
            "T",
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
    let outcome = db.batch_close_tasks(&[t.id, t.id], false).unwrap();
    assert_eq!(outcome.closed.len(), 1, "closed once");
    assert!(
        outcome.not_found.is_empty(),
        "no phantom not_found from the dup"
    );
    assert!(outcome.already_closed.is_empty());
}
