//! Integration tests for mycelium-core against an in-memory database.
//! Covers CRUD round-trips and the serde representation of the enums (which
//! must match the on-disk DB strings, e.g. "in_progress").

use mycelium_core::models::{FollowupStatus, Priority, Status, TaskRefType};
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
        .list_tasks(Some(e1.id), None, None, None, false, false, None, None)
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

#[test]
fn update_task_cannot_close_a_blocked_task() {
    // Regression: the blocker guard used to live only in the `close` command,
    // so `myc task update <id> --status closed` closed a blocked task with no
    // warning, leaving the incoherent state "closed" + "blocked by #N".
    // The guard now lives in update_task itself.
    let mut db = db();
    let blocked = db
        .create_task(
            "Blocked",
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
    let blocker = db
        .create_task(
            "Blocker",
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
    db.add_dependency(blocked.id, blocker.id).unwrap();

    let err = db
        .update_task(
            blocked.id,
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
        .expect_err("closing a blocked task via update_task must be refused");
    assert!(
        matches!(err, mycelium_core::error::MyceliumError::BlockedBy(_)),
        "expected BlockedBy, got {err:?}"
    );
    assert_eq!(
        db.get_task(blocked.id).unwrap().unwrap().status,
        Status::Open,
        "the refused update must not have written anything"
    );

    // Non-status edits on a blocked task stay allowed.
    db.update_task(
        blocked.id,
        Some("Renamed while blocked"),
        None,
        None,
        Some(Priority::High),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("editing other fields of a blocked task is fine");

    // The explicit override still works.
    db.update_task_forced(
        blocked.id,
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
    .expect("update_task_forced bypasses the guard");
    assert_eq!(
        db.get_task(blocked.id).unwrap().unwrap().status,
        Status::Closed
    );

    // Once the blocker is closed, the plain path works again.
    let other = db
        .create_task(
            "Other blocked",
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
    db.add_dependency(other.id, blocker.id).unwrap();
    db.update_task(
        blocker.id,
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
    .expect("the blocker itself has no blockers");
    db.update_task(
        other.id,
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
    .expect("no open blockers left -> close is allowed");
}

// --- FK reference validation -------------------------------------------
// `tasks.epic_id`/`assignee_id` are real FKs with `PRAGMA foreign_keys = ON`,
// so a dangling id always failed — but as a raw
// "FOREIGN KEY constraint failed" naming neither column nor id. These tests
// pin the actionable error; if they start seeing the SQLite text again, the
// up-front check was lost.

fn assert_not_found(err: mycelium_core::error::MyceliumError, entity: &str, id: &str) {
    match err {
        mycelium_core::error::MyceliumError::NotFound { entity: e, id: got } => {
            assert_eq!(e, entity, "wrong entity in NotFound");
            assert_eq!(got, id, "wrong id in NotFound");
        }
        other => panic!("expected NotFound({entity} #{id}), got: {other}"),
    }
}

#[test]
fn create_task_rejects_missing_epic() {
    let mut db = db();
    let err = db
        .create_task(
            "orphan",
            None,
            Some(73),
            Priority::High,
            None,
            None,
            None,
            None,
            None,
        )
        .expect_err("epic #73 does not exist");
    assert_not_found(err, "epic", "73");
}

#[test]
fn create_task_rejects_missing_assignee() {
    let mut db = db();
    let err = db
        .create_task(
            "orphan",
            None,
            None,
            Priority::Medium,
            Some(404),
            None,
            None,
            None,
            None,
        )
        .expect_err("assignee #404 does not exist");
    assert_not_found(err, "assignee", "404");
}

#[test]
fn create_task_accepts_valid_and_absent_refs() {
    let mut db = db();
    let epic = db.create_epic("Real", None, None, None).unwrap();
    let who = db.create_assignee("Ada", None, None).unwrap();

    let t = db
        .create_task(
            "linked",
            None,
            Some(epic.id),
            Priority::High,
            Some(who.id),
            None,
            None,
            None,
            None,
        )
        .expect("valid refs are accepted");
    assert_eq!(t.epic_id, Some(epic.id));
    assert_eq!(t.assignee_id, Some(who.id));

    // No refs at all is still legal — validation must not reject None.
    db.create_task(
        "free",
        None,
        None,
        Priority::Low,
        None,
        None,
        None,
        None,
        None,
    )
    .expect("absent refs are accepted");
}

#[test]
fn update_task_rejects_missing_epic_without_partial_write() {
    let mut db = db();
    let t = db
        .create_task(
            "before",
            None,
            None,
            Priority::Low,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

    // Title and epic in one call. The field UPDATEs are separate statements,
    // so if the epic were only caught by SQLite the title would already be
    // committed. Nothing may change.
    let err = db
        .update_task(
            t.id,
            Some("after"),
            None,
            None,
            None,
            Some(Some(73)),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect_err("epic #73 does not exist");
    assert_not_found(err, "epic", "73");

    let unchanged = db.get_task(t.id).unwrap().expect("task still there");
    assert_eq!(
        unchanged.title, "before",
        "failed update must not leave a partial write"
    );
    assert_eq!(unchanged.epic_id, None);
}

#[test]
fn update_task_can_still_clear_refs() {
    let mut db = db();
    let epic = db.create_epic("E", None, None, None).unwrap();
    let t = db
        .create_task(
            "t",
            None,
            Some(epic.id),
            Priority::Low,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

    // Some(None) means "clear it" — there is no id to look up, so this must
    // not be mistaken for a dangling reference.
    let cleared = db
        .update_task(
            t.id,
            None,
            None,
            None,
            None,
            Some(None),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .expect("clearing the epic is allowed");
    assert_eq!(cleared.epic_id, None);
}

// ---- Task hierarchy (parent/child subtasks) ----

/// Make a bare task with just a title, returning its id. Keeps the ref/parent
/// tests readable.
fn mk(db: &mut Database, title: &str) -> i64 {
    db.create_task(
        title,
        None,
        None,
        Priority::Medium,
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap()
    .id
}

#[test]
fn search_tasks_matches_title_and_description() {
    let mut db = db();
    let t1 = db
        .create_task(
            "implement SP subquery",
            Some("backend work"),
            None,
            Priority::Medium,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap()
        .id;
    let t2 = db
        .create_task(
            "FE custom_field",
            Some("missing organization_custom_field_id"),
            None,
            Priority::Medium,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap()
        .id;

    // Title match.
    let by_title = db.search_tasks("subquery").unwrap();
    assert_eq!(by_title.iter().map(|t| t.id).collect::<Vec<_>>(), vec![t1]);

    // Description-only match (word appears only in t2's description).
    let by_desc = db.search_tasks("organization").unwrap();
    assert_eq!(by_desc.iter().map(|t| t.id).collect::<Vec<_>>(), vec![t2]);

    // Case-insensitive.
    assert_eq!(db.search_tasks("SUBQUERY").unwrap().len(), 1);

    // No match.
    assert!(db.search_tasks("zzznotfound").unwrap().is_empty());

    // Empty query returns nothing (never "match everything").
    assert!(db.search_tasks("").unwrap().is_empty());
    assert!(db.search_tasks("   ").unwrap().is_empty());
}

#[test]
fn list_tasks_filters_by_parent() {
    let mut db = db();
    let p = mk(&mut db, "parent");
    let c1 = mk(&mut db, "child1");
    let c2 = mk(&mut db, "child2");
    let top = mk(&mut db, "top-level sibling");
    db.set_parent(c1, Some(p)).unwrap();
    db.set_parent(c2, Some(p)).unwrap();

    // Some(p) -> only the direct children of p.
    let children = db
        .list_tasks(None, None, None, None, false, false, None, Some(p))
        .unwrap();
    let mut ids: Vec<i64> = children.iter().map(|t| t.id).collect();
    ids.sort();
    assert_eq!(ids, vec![c1, c2]);

    // Some(0) -> only top-level tasks (no parent): p and top, NOT the children.
    let tops = db
        .list_tasks(None, None, None, None, false, false, None, Some(0))
        .unwrap();
    let mut top_ids: Vec<i64> = tops.iter().map(|t| t.id).collect();
    top_ids.sort();
    assert_eq!(top_ids, vec![p, top]);

    // None -> no parent filter, everything shows.
    let all = db
        .list_tasks(None, None, None, None, false, false, None, None)
        .unwrap();
    assert_eq!(all.len(), 4);
}

#[test]
fn set_parent_and_detach() {
    let mut db = db();
    let parent = mk(&mut db, "parent");
    let child = mk(&mut db, "child");

    db.set_parent(child, Some(parent)).unwrap();
    assert_eq!(db.get_task(child).unwrap().unwrap().parent_id, Some(parent));
    assert_eq!(
        db.get_children(parent)
            .unwrap()
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>(),
        vec![child]
    );

    // Detach (0 -> None at the CLI boundary; core takes None).
    db.set_parent(child, None).unwrap();
    assert_eq!(db.get_task(child).unwrap().unwrap().parent_id, None);
    assert!(db.get_children(parent).unwrap().is_empty());
}

#[test]
fn set_parent_rejects_self_and_cycles() {
    let mut db = db();
    let a = mk(&mut db, "a");
    let b = mk(&mut db, "b");
    let c = mk(&mut db, "c");

    // self-parent
    assert!(db.set_parent(a, Some(a)).is_err());

    // a -> b -> c chain, then try c as ancestor of a (would cycle)
    db.set_parent(b, Some(a)).unwrap();
    db.set_parent(c, Some(b)).unwrap();
    // making a a child of c closes the loop a->b->c->a
    assert!(
        db.set_parent(a, Some(c)).is_err(),
        "cycle through the parent chain must be rejected"
    );
    // the rejected write must not have persisted
    assert_eq!(db.get_task(a).unwrap().unwrap().parent_id, None);
}

#[test]
fn set_parent_missing_task_or_parent_errors() {
    let mut db = db();
    let a = mk(&mut db, "a");
    assert!(db.set_parent(9999, Some(a)).is_err(), "missing child");
    assert!(db.set_parent(a, Some(9999)).is_err(), "missing parent");
}

#[test]
fn open_children_excludes_closed() {
    let mut db = db();
    let parent = mk(&mut db, "parent");
    let c1 = mk(&mut db, "c1");
    let c2 = mk(&mut db, "c2");
    db.set_parent(c1, Some(parent)).unwrap();
    db.set_parent(c2, Some(parent)).unwrap();
    // close c2 via update_task status
    db.update_task(
        c2,
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
    let open = db.get_open_children(parent).unwrap();
    assert_eq!(open.iter().map(|t| t.id).collect::<Vec<_>>(), vec![c1]);
}

// ---- Non-blocking task references (relates / duplicate) ----

#[test]
fn task_ref_is_symmetric() {
    let mut db = db();
    let a = mk(&mut db, "a");
    let b = mk(&mut db, "b");

    // create as relates(a, b); refs from B's side must show A.
    db.add_task_ref(a, b, TaskRefType::Relates).unwrap();
    let from_b = db.get_task_refs(b).unwrap();
    assert_eq!(from_b.len(), 1);
    assert_eq!(from_b[0].other_id, a);
    assert_eq!(from_b[0].ref_type, TaskRefType::Relates);

    let from_a = db.get_task_refs(a).unwrap();
    assert_eq!(from_a[0].other_id, b);
}

#[test]
fn task_ref_is_idempotent_both_directions() {
    let mut db = db();
    let a = mk(&mut db, "a");
    let b = mk(&mut db, "b");
    db.add_task_ref(a, b, TaskRefType::Relates).unwrap();
    // same link, reversed args, same type -> no second row
    db.add_task_ref(b, a, TaskRefType::Relates).unwrap();
    assert_eq!(db.get_task_refs(a).unwrap().len(), 1);
    // but a DIFFERENT type is a distinct link
    db.add_task_ref(a, b, TaskRefType::Duplicate).unwrap();
    assert_eq!(db.get_task_refs(a).unwrap().len(), 2);
}

#[test]
fn task_ref_rejects_self_and_missing() {
    let mut db = db();
    let a = mk(&mut db, "a");
    assert!(
        db.add_task_ref(a, a, TaskRefType::Relates).is_err(),
        "self-ref"
    );
    assert!(
        db.add_task_ref(a, 9999, TaskRefType::Relates).is_err(),
        "missing other"
    );
}

#[test]
fn remove_task_ref_both_directions() {
    let mut db = db();
    let a = mk(&mut db, "a");
    let b = mk(&mut db, "b");
    db.add_task_ref(a, b, TaskRefType::Duplicate).unwrap();
    // remove using reversed args must still delete the single stored row
    let n = db.remove_task_ref(b, a, TaskRefType::Duplicate).unwrap();
    assert_eq!(n, 1);
    assert!(db.get_task_refs(a).unwrap().is_empty());
    // removing again matches nothing
    assert_eq!(db.remove_task_ref(a, b, TaskRefType::Duplicate).unwrap(), 0);
}

#[test]
fn task_ref_does_not_block_state_transitions() {
    // The whole point of a separate table: a relates/duplicate ref must NOT
    // register as a blocker, so a task with only refs can still move states.
    let mut db = db();
    let a = mk(&mut db, "a");
    let b = mk(&mut db, "b");
    db.add_task_ref(a, b, TaskRefType::Relates).unwrap();
    db.add_task_ref(a, b, TaskRefType::Duplicate).unwrap();
    assert!(
        db.get_open_blockers(a).unwrap().is_empty(),
        "refs are not blockers"
    );
    // moving to in_progress / closed must succeed (no phantom blocker)
    db.update_task(
        a,
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
    .expect("refs must not gate transitions");
}
