use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Concurrent `task create` invocations must not drop writes. Under WAL, SQLite
/// serializes writers with a single lock; without a busy_timeout the second
/// writer gets SQLITE_BUSY immediately and the insert is lost. This spawns
/// several creates in parallel and asserts every one landed.
#[test]
fn test_concurrent_task_create_no_lost_writes() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("init");
    myc_cmd(&temp)
        .args(["epic", "create", "--title", "E"])
        .output()
        .expect("epic");

    // Fire N creates concurrently (spawn without waiting).
    let n = 6;
    let mut children = Vec::new();
    for i in 0..n {
        let child = myc_cmd(&temp)
            .args(["task", "create", "--title", &format!("T{i}"), "--epic", "1"])
            .spawn()
            .expect("spawn create");
        children.push(child);
    }
    for mut child in children {
        let status = child.wait().expect("wait");
        assert!(status.success(), "a concurrent create failed with {status}");
    }

    let out = myc_cmd(&temp)
        .args(["task", "list", "--all", "--format", "json"])
        .output()
        .expect("list");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let count = stdout.matches("\"id\"").count();
    assert_eq!(count, n, "expected {n} tasks, got {count}: {stdout}");
}

fn myc_path() -> PathBuf {
    // First try CARGO_BIN_EXE_myc
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_myc") {
        return PathBuf::from(path);
    }

    let debug_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("myc");
    if debug_path.exists() {
        return debug_path;
    }

    // Otherwise fall back to release
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release")
        .join("myc")
}

fn myc_cmd(temp_dir: &TempDir) -> Command {
    let mut cmd = Command::new(myc_path());
    cmd.current_dir(temp_dir);
    cmd
}

fn print_output(output: &std::process::Output) {
    eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
    eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    eprintln!("status: {}", output.status);
}

#[test]
fn test_init() {
    let temp = TempDir::new().unwrap();
    let output = myc_cmd(&temp)
        .arg("init")
        .output()
        .expect("Failed to execute");
    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Mycelium project initialized"));
}

/// Schema-equality guard between the CLI (`myc`) migrations and mycui's
/// `init_schema` (mycui/src-tauri/src/db.rs). The two crates own separate
/// schema code today (follow-up #1 tracks unifying them via a shared
/// `mycelium-core` crate). Until then, this test fails loudly the moment
/// either side changes a shared table/column, so drift can't ship silently.
///
/// Invariant: every table + column mycui creates must exist in the CLI DB
/// with the same declared type. The CLI may have MORE (task_notes, epic_notes,
/// linear_sync, v5 columns) — that's expected, mycui just doesn't read them.
///
/// If this fails: mirror the change on both sides, or land follow-up #1.
#[test]
fn test_schema_matches_mycui_init_schema() {
    // Canonical expectation mirrored from mycui/src-tauri/src/db.rs::init_schema.
    // (table, [(column, declared_type)]). Keep in lockstep with that file.
    let expected: &[(&str, &[(&str, &str)])] = &[
        (
            "epics",
            &[
                ("id", "INTEGER"),
                ("title", "TEXT"),
                ("description", "TEXT"),
                ("status", "TEXT"),
                ("created_at", "TEXT"),
                ("updated_at", "TEXT"),
            ],
        ),
        (
            "assignees",
            &[
                ("id", "INTEGER"),
                ("name", "TEXT"),
                ("email", "TEXT"),
                ("github_username", "TEXT"),
                ("created_at", "TEXT"),
            ],
        ),
        (
            "tasks",
            &[
                ("id", "INTEGER"),
                ("title", "TEXT"),
                ("description", "TEXT"),
                ("status", "TEXT"),
                ("priority", "TEXT"),
                ("epic_id", "INTEGER"),
                ("assignee_id", "INTEGER"),
                ("due_date", "TEXT"),
                ("tags", "TEXT"),
                ("created_at", "TEXT"),
                ("updated_at", "TEXT"),
            ],
        ),
        (
            "dependencies",
            &[
                ("id", "INTEGER"),
                ("task_id", "INTEGER"),
                ("depends_on_task_id", "INTEGER"),
                ("created_at", "TEXT"),
            ],
        ),
        (
            "followups",
            &[
                ("id", "INTEGER"),
                ("body", "TEXT"),
                ("title", "TEXT"),
                ("status", "TEXT"),
                ("closure_reason", "TEXT"),
                ("created_at", "TEXT"),
                ("closed_at", "TEXT"),
            ],
        ),
    ];

    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");
    let db_path = temp.path().join(".mycelium").join("mycelium.db");
    assert!(db_path.exists(), "myc init did not create the database");

    let conn = rusqlite::Connection::open(&db_path).expect("open cli db");

    for (table, columns) in expected {
        // (column_name -> declared_type) from the CLI schema.
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("prepare pragma");
        let actual: std::collections::HashMap<String, String> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .expect("query pragma")
            .map(|r| r.expect("row"))
            .collect();

        assert!(
            !actual.is_empty(),
            "CLI schema is missing table `{table}` that mycui creates"
        );

        for (col, ty) in *columns {
            match actual.get(*col) {
                None => panic!(
                    "CLI schema table `{table}` is missing column `{col}` \
                     (mycui expects it). Schemas have drifted — sync both sides \
                     or land follow-up #1 (shared mycelium-core crate)."
                ),
                Some(actual_ty) => assert_eq!(
                    actual_ty.to_uppercase(),
                    ty.to_uppercase(),
                    "column `{table}.{col}` type drift: CLI has `{actual_ty}`, \
                     mycui expects `{ty}`"
                ),
            }
        }
    }
}

#[test]
fn test_epic_create() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");

    let output = myc_cmd(&temp)
        .arg("epic")
        .arg("create")
        .arg("--title")
        .arg("Test Epic")
        .output()
        .expect("Failed to create epic");

    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created epic #1"));
}

#[test]
fn test_task_create() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");
    myc_cmd(&temp)
        .arg("epic")
        .arg("create")
        .arg("--title")
        .arg("Test Epic")
        .output()
        .expect("Failed to create epic");

    let output = myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Test Task")
        .arg("--epic")
        .arg("1")
        .output()
        .expect("Failed to create task");

    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created task #1"));
}

#[test]
fn test_dependency_blocking() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");
    myc_cmd(&temp)
        .arg("epic")
        .arg("create")
        .arg("--title")
        .arg("Test Epic")
        .output()
        .expect("Failed to create epic");
    myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Task 1")
        .output()
        .expect("Failed to create task 1");
    myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Task 2")
        .output()
        .expect("Failed to create task 2");

    // Make task 1 block task 2
    let output = myc_cmd(&temp)
        .arg("task")
        .arg("link")
        .arg("blocks")
        .arg("--task")
        .arg("1")
        .arg("2")
        .output()
        .expect("Failed to link");

    print_output(&output);
    assert!(output.status.success());

    // Try to close task 2 (should fail since it's blocked)
    let output = myc_cmd(&temp)
        .arg("task")
        .arg("close")
        .arg("2")
        .output()
        .expect("Failed to close");

    print_output(&output);
    // Note: blocked message goes to stdout since it's not a hard error
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("blocked by"));

    // Close task 1 first
    let output = myc_cmd(&temp)
        .arg("task")
        .arg("close")
        .arg("1")
        .output()
        .expect("Failed to close 1");
    print_output(&output);
    assert!(output.status.success());

    // Now task 2 should close
    let output = myc_cmd(&temp)
        .arg("task")
        .arg("close")
        .arg("2")
        .output()
        .expect("Failed to close 2");
    print_output(&output);
    assert!(output.status.success());
}

#[test]
fn test_assignee() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");

    // Create assignee
    let output = myc_cmd(&temp)
        .arg("assignee")
        .arg("create")
        .arg("--name")
        .arg("John Doe")
        .arg("--github")
        .arg("johndoe")
        .output()
        .expect("Failed to create assignee");

    print_output(&output);
    assert!(output.status.success());

    // Create task and assign
    myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Test")
        .output()
        .expect("Failed to create task");

    let output = myc_cmd(&temp)
        .arg("task")
        .arg("assign")
        .arg("1")
        .arg("1")
        .output()
        .expect("Failed to assign");

    print_output(&output);
    assert!(output.status.success());
}

#[test]
fn test_json_output() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");
    myc_cmd(&temp)
        .arg("epic")
        .arg("create")
        .arg("--title")
        .arg("Test Epic")
        .output()
        .expect("Failed to create epic");

    let output = myc_cmd(&temp)
        .arg("epic")
        .arg("list")
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to list");

    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("{")); // Should be JSON
}

#[test]
fn test_export() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");
    myc_cmd(&temp)
        .arg("epic")
        .arg("create")
        .arg("--title")
        .arg("Test Epic")
        .output()
        .expect("Failed to create epic");
    myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Test Task")
        .output()
        .expect("Failed to create task");

    let output = myc_cmd(&temp)
        .arg("export")
        .arg("json")
        .output()
        .expect("Failed to export");

    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("epics"));
    assert!(stdout.contains("tasks"));
}

#[test]
fn test_task_create_help_does_not_panic() {
    let temp = TempDir::new().unwrap();
    let output = myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--help")
        .output()
        .expect("Failed to show help");

    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Create a new task"));
}

#[test]
fn test_blocked_list_only_shows_tasks_with_open_blockers() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");
    myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Blocker")
        .output()
        .expect("Failed to create blocker");
    myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Blocked")
        .output()
        .expect("Failed to create blocked task");
    myc_cmd(&temp)
        .arg("task")
        .arg("link")
        .arg("blocks")
        .arg("--task")
        .arg("1")
        .arg("2")
        .output()
        .expect("Failed to link dependency");

    let output = myc_cmd(&temp)
        .arg("task")
        .arg("list")
        .arg("--blocked")
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to list blocked tasks");
    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"id\": 2"));

    myc_cmd(&temp)
        .arg("task")
        .arg("close")
        .arg("1")
        .output()
        .expect("Failed to close blocker");

    let output = myc_cmd(&temp)
        .arg("task")
        .arg("list")
        .arg("--blocked")
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to list blocked tasks after closing blocker");
    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "[]");
}

#[test]
fn test_delete_epic_detaches_tasks_instead_of_deleting_them() {
    let temp = TempDir::new().unwrap();
    myc_cmd(&temp).arg("init").output().expect("Failed to init");
    myc_cmd(&temp)
        .arg("epic")
        .arg("create")
        .arg("--title")
        .arg("Epic")
        .output()
        .expect("Failed to create epic");
    myc_cmd(&temp)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Task")
        .arg("--epic")
        .arg("1")
        .output()
        .expect("Failed to create task");

    let output = myc_cmd(&temp)
        .arg("epic")
        .arg("delete")
        .arg("1")
        .arg("--force")
        .output()
        .expect("Failed to delete epic");
    print_output(&output);
    assert!(output.status.success());

    let output = myc_cmd(&temp)
        .arg("task")
        .arg("show")
        .arg("1")
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to show task");
    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"id\": 1"));
    assert!(stdout.contains("\"epic_id\": null"));
}

#[test]
fn test_followup_full_lifecycle() {
    let temp = TempDir::new().unwrap();
    let _ = myc_cmd(&temp).arg("init").output().expect("init");

    // add (with title)
    let out = myc_cmd(&temp)
        .args(["followup", "add", "first body text", "--title", "first"])
        .output()
        .expect("add");
    print_output(&out);
    assert!(out.status.success());

    // add (no title, alias `fu`)
    let out = myc_cmd(&temp)
        .args(["fu", "add", "second body, no title"])
        .output()
        .expect("add 2");
    assert!(out.status.success());

    // list — default -a shows all (both visible)
    let out = myc_cmd(&temp)
        .args(["followup", "list", "--format", "json"])
        .output()
        .expect("list");
    let json = String::from_utf8_lossy(&out.stdout);
    assert!(json.contains("\"id\": 1"));
    assert!(json.contains("\"id\": 2"));

    // -o shows only active items (both open at this point)
    let out = myc_cmd(&temp)
        .args(["followup", "list", "-o", "--format", "json"])
        .output()
        .expect("list -o");
    let json = String::from_utf8_lossy(&out.stdout);
    assert!(json.contains("\"id\": 1"));
    assert!(json.contains("\"id\": 2"));

    // -c shows only closed items (none yet)
    let out = myc_cmd(&temp)
        .args(["followup", "list", "-c", "--format", "json"])
        .output()
        .expect("list -c");
    let json = String::from_utf8_lossy(&out.stdout);
    assert!(!json.contains("\"id\": 1"));
    assert!(!json.contains("\"id\": 2"));

    // count
    let out = myc_cmd(&temp)
        .args(["followup", "count", "--format", "json"])
        .output()
        .expect("count");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(body.contains("\"open\": 2"));

    // append
    let out = myc_cmd(&temp)
        .args(["followup", "append", "1", "extra context"])
        .output()
        .expect("append");
    assert!(out.status.success());
    let out = myc_cmd(&temp)
        .args(["followup", "show", "1", "--format", "json"])
        .output()
        .expect("show");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(body.contains("extra context"));

    // edit replace
    let out = myc_cmd(&temp)
        .args(["followup", "edit", "1", "--body", "fully replaced"])
        .output()
        .expect("edit");
    assert!(out.status.success());

    // start + done
    let out = myc_cmd(&temp)
        .args(["followup", "start", "1"])
        .output()
        .expect("start");
    assert!(out.status.success());
    let out = myc_cmd(&temp)
        .args(["followup", "done", "1", "--reason", "tested"])
        .output()
        .expect("done");
    assert!(out.status.success());

    // next should now return #2 (only remaining open)
    let out = myc_cmd(&temp)
        .args(["followup", "next", "--format", "json"])
        .output()
        .expect("next");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(body.contains("\"id\": 2"));

    // -c now shows #1 (done), -o shows #2 (open)
    let out = myc_cmd(&temp)
        .args(["followup", "list", "-c", "--format", "json"])
        .output()
        .expect("list -c");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(body.contains("\"id\": 1"));
    assert!(!body.contains("\"id\": 2"));
    let out = myc_cmd(&temp)
        .args(["followup", "list", "-o", "--format", "json"])
        .output()
        .expect("list -o");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(!body.contains("\"id\": 1"));
    assert!(body.contains("\"id\": 2"));

    // wontfix #2
    let out = myc_cmd(&temp)
        .args(["followup", "wontfix", "2", "--reason", "scope creep"])
        .output()
        .expect("wontfix");
    assert!(out.status.success());

    // next should be null now
    let out = myc_cmd(&temp)
        .args(["followup", "next", "--format", "json"])
        .output()
        .expect("next empty");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(body.trim().starts_with("null"));

    // reopen #2
    let out = myc_cmd(&temp)
        .args(["followup", "reopen", "2"])
        .output()
        .expect("reopen");
    assert!(out.status.success());

    // rm with --force
    let out = myc_cmd(&temp)
        .args(["followup", "rm", "2", "--force"])
        .output()
        .expect("rm");
    assert!(out.status.success());
}

#[test]
fn test_followup_promote_to_task() {
    let temp = TempDir::new().unwrap();
    let _ = myc_cmd(&temp).arg("init").output().expect("init");

    let _ = myc_cmd(&temp)
        .args([
            "followup",
            "add",
            "should become a real task",
            "--title",
            "Real work",
        ])
        .output()
        .expect("add");

    let out = myc_cmd(&temp)
        .args(["followup", "promote", "1", "--priority", "high"])
        .output()
        .expect("promote");
    print_output(&out);
    assert!(out.status.success());

    // Followup should now be done
    let out = myc_cmd(&temp)
        .args(["followup", "show", "1", "--format", "json"])
        .output()
        .expect("show fu");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(body.contains("\"status\": \"done\""));
    assert!(body.contains("Promoted to task #1"));

    // Task should exist with the title
    let out = myc_cmd(&temp)
        .args(["task", "show", "1", "--format", "json"])
        .output()
        .expect("show task");
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(body.contains("\"title\": \"Real work\""));
    assert!(body.contains("\"priority\": \"high\""));
}

#[test]
fn test_followup_close_hint_fires() {
    let temp = TempDir::new().unwrap();
    let _ = myc_cmd(&temp).arg("init").output().expect("init");
    let _ = myc_cmd(&temp)
        .args(["task", "create", "--title", "Some task"])
        .output()
        .expect("task create");
    let _ = myc_cmd(&temp)
        .args(["followup", "add", "something to look at later"])
        .output()
        .expect("fu add");

    let out = myc_cmd(&temp)
        .args(["task", "close", "1"])
        .output()
        .expect("task close");
    print_output(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("open follow-up"));
}
