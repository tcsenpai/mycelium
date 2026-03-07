use std::process::Command;
use std::path::PathBuf;
use tempfile::TempDir;

fn myc_path() -> PathBuf {
    // First try CARGO_BIN_EXE_myc
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_myc") {
        return PathBuf::from(path);
    }
    // Otherwise assume we're in the target directory
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
    let output = myc_cmd(&temp).arg("init").output().expect("Failed to execute");
    print_output(&output);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Mycelium project initialized"));
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
    myc_cmd(&temp).arg("epic").arg("create").arg("--title").arg("Test Epic").output().expect("Failed to create epic");
    
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
    myc_cmd(&temp).arg("epic").arg("create").arg("--title").arg("Test Epic").output().expect("Failed to create epic");
    myc_cmd(&temp).arg("task").arg("create").arg("--title").arg("Task 1").output().expect("Failed to create task 1");
    myc_cmd(&temp).arg("task").arg("create").arg("--title").arg("Task 2").output().expect("Failed to create task 2");
    
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
    let output = myc_cmd(&temp).arg("task").arg("close").arg("1").output().expect("Failed to close 1");
    print_output(&output);
    assert!(output.status.success());
    
    // Now task 2 should close
    let output = myc_cmd(&temp).arg("task").arg("close").arg("2").output().expect("Failed to close 2");
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
    myc_cmd(&temp).arg("task").arg("create").arg("--title").arg("Test").output().expect("Failed to create task");
    
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
    myc_cmd(&temp).arg("epic").arg("create").arg("--title").arg("Test Epic").output().expect("Failed to create epic");
    
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
    myc_cmd(&temp).arg("epic").arg("create").arg("--title").arg("Test Epic").output().expect("Failed to create epic");
    myc_cmd(&temp).arg("task").arg("create").arg("--title").arg("Test Task").output().expect("Failed to create task");
    
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
