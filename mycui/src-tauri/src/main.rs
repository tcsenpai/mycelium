// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tauri::{Manager, Emitter, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::{Shortcut, Code, Modifiers};
use tauri_plugin_dialog::DialogExt;

mod dto;

use dto::*;
use mycelium_core::Database;
use std::collections::HashMap;

const MAX_RECENT_FOLDERS: usize = 10;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct AppConfig {
    recent_folders: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            recent_folders: Vec::new(),
        }
    }
}

// App state shared across commands
struct AppState {
    db: Arc<Mutex<Database>>,
    current_db_path: Arc<Mutex<Option<std::path::PathBuf>>>,
    db_watch_generation: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileSignature {
    modified_millis: u128,
    len: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct DbSignature {
    db: Option<FileSignature>,
    wal: Option<FileSignature>,
    shm: Option<FileSignature>,
}

fn find_mycelium_db() -> Option<std::path::PathBuf> {
    // Check current directory first
    let current = std::env::current_dir().ok()?;
    let db_path = current.join(".mycelium").join("mycelium.db");
    if db_path.exists() {
        return Some(db_path);
    }

    // Check parent directories
    let mut path = current;
    while let Some(parent) = path.parent() {
        let db_path = parent.join(".mycelium").join("mycelium.db");
        if db_path.exists() {
            return Some(db_path);
        }
        path = parent.to_path_buf();
    }

    // Check home directory
    if let Some(home) = dirs::home_dir() {
        let db_path = home.join(".mycelium").join("mycelium.db");
        if db_path.exists() {
            return Some(db_path);
        }
    }

    None
}

fn add_to_recent_folders(app_dir: &std::path::Path, path: String) {
    let _ = std::fs::create_dir_all(app_dir);
    let config_path: std::path::PathBuf = app_dir.join("config.json");
    
    let mut config: AppConfig = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(c) => match serde_json::from_str(&c) {
                Ok(cfg) => cfg,
                Err(e) => {
                    // The file exists but is corrupt. Falling back to
                    // default() here would overwrite the user's entire
                    // recent-folders history with an empty list on the write
                    // below. Abort instead — preserve whatever is on disk.
                    eprintln!(
                        "mycui: config.json is corrupt ({e}); not overwriting to avoid losing recent folders"
                    );
                    return;
                }
            },
            Err(e) => {
                // Transient read failure — don't clobber existing config.
                eprintln!("mycui: could not read config.json ({e}); skipping recent-folders update");
                return;
            }
        }
    } else {
        AppConfig::default()
    };

    // Remove if already exists
    config.recent_folders.retain(|f| f != &path);

    // Add to front
    config.recent_folders.insert(0, path);

    // Limit to max
    config.recent_folders.truncate(MAX_RECENT_FOLDERS);

    // Serialize before writing: an empty string from a failed serialize would
    // truncate the file, so bail on error rather than writing garbage.
    match serde_json::to_string_pretty(&config) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&config_path, json) {
                eprintln!("mycui: failed to write config.json ({e})");
            }
        }
        Err(e) => eprintln!("mycui: failed to serialize config ({e}); recent folders not saved"),
    }
}

fn get_recent_folders_from_disk(app_dir: &std::path::Path) -> Vec<String> {
    let config_path: std::path::PathBuf = app_dir.join("config.json");
    
    if !config_path.exists() {
        return Vec::new();
    }
    
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
    // A corrupt config yields an empty list for this read only — do NOT persist
    // that here; the write path (add_to_recent_folders) refuses to overwrite a
    // corrupt file, so the history survives. Log so it isn't silent.
    let config: AppConfig = serde_json::from_str(&content).unwrap_or_else(|e| {
        if !content.is_empty() {
            eprintln!("mycui: config.json is corrupt ({e}); showing no recent folders this run");
        }
        AppConfig::default()
    });

    // Filter out non-existent folders
    config.recent_folders
        .into_iter()
        .filter(|f| {
            let path = std::path::PathBuf::from(f);
            path.exists() && path.join(".mycelium").join("mycelium.db").exists()
        })
        .collect()
}

fn file_signature(path: &std::path::Path) -> Option<FileSignature> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let modified_millis = modified.duration_since(UNIX_EPOCH).ok()?.as_millis();

    Some(FileSignature {
        modified_millis,
        len: metadata.len(),
    })
}

fn project_db_signature(project_path: &std::path::Path) -> DbSignature {
    let db_dir = project_path.join(".mycelium");
    DbSignature {
        db: file_signature(&db_dir.join("mycelium.db")),
        wal: file_signature(&db_dir.join("mycelium.db-wal")),
        shm: file_signature(&db_dir.join("mycelium.db-shm")),
    }
}

fn spawn_db_watch(
    app_handle: tauri::AppHandle,
    project_path: std::path::PathBuf,
    watch_generation: Arc<AtomicU64>,
) {
    let generation = watch_generation.fetch_add(1, Ordering::SeqCst) + 1;

    tauri::async_runtime::spawn(async move {
        let mut last_signature = project_db_signature(&project_path);

        loop {
            tokio::time::sleep(Duration::from_millis(750)).await;

            if watch_generation.load(Ordering::SeqCst) != generation {
                break;
            }

            let next_signature = project_db_signature(&project_path);
            if next_signature != last_signature {
                last_signature = next_signature;
                let _ = app_handle.emit("database-changed", ());
            }
        }
    });
}

#[tauri::command]
async fn open_folder_dialog(
    app_handle: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let folder = app_handle.dialog()
        .file()
        .set_title("Select Project Folder")
        .blocking_pick_folder();
    
    Ok(folder.map(|p| p.to_string()))
}

#[tauri::command]
async fn get_current_db_path(
    state: tauri::State<'_, AppState>
) -> Result<Option<String>, String> {
    let path = state.current_db_path.lock().await;
    Ok(path.as_ref().map(|p| p.to_string_lossy().to_string()))
}

/// Locate the `claude` CLI. A GUI app launched from Finder/Dock does not
/// inherit the shell's PATH, so `Command::new("claude")` fails there even
/// though it works when the app is started from a terminal.
fn resolve_claude_binary() -> Option<std::path::PathBuf> {
    if let Ok(explicit) = std::env::var("CLAUDE_BINARY") {
        let path = std::path::PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("claude");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Common install locations, for the Finder-launch case above.
    let mut fallbacks: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        fallbacks.push(home.join(".local/bin/claude"));
        fallbacks.push(home.join(".claude/local/claude"));
        fallbacks.push(home.join(".bun/bin/claude"));
        fallbacks.push(home.join(".npm-global/bin/claude"));
    }
    fallbacks.push(std::path::PathBuf::from("/opt/homebrew/bin/claude"));
    fallbacks.push(std::path::PathBuf::from("/usr/local/bin/claude"));

    fallbacks.into_iter().find(|p| p.is_file())
}

#[tauri::command]
async fn claude_available() -> Result<bool, String> {
    Ok(resolve_claude_binary().is_some())
}

/// Run a one-shot `claude -p` query. The prompt (project context + question)
/// is written to stdin rather than passed as an argument: task titles are
/// arbitrary user text, and putting them in argv risks hitting the platform
/// argument-length limit on large projects.
#[tauri::command]
async fn ask_claude(prompt: String, model: Option<String>) -> Result<String, String> {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;

    let binary = resolve_claude_binary()
        .ok_or_else(|| "claude CLI not found. Install Claude Code, or set CLAUDE_BINARY to its full path.".to_string())?;

    let mut command = tokio::process::Command::new(binary);
    command.arg("-p");
    // Read-only by construction. The chat answers questions about project data
    // that is already embedded in the prompt, so it never needs to touch the
    // filesystem or run commands. `--tools ""` disables the whole built-in set
    // (Read/Edit/Write/Bash/...), and `dontAsk` guarantees a headless process
    // can't be silently granted anything: with no TTY there is nobody to
    // approve a prompt, so any tool request fails closed instead of hanging.
    // Verified: asking it to write a file or run `touch` under these flags
    // produces a refusal and leaves the filesystem untouched.
    command.arg("--tools").arg("");
    command.arg("--permission-mode").arg("dontAsk");
    if let Some(model) = model.as_deref().filter(|m| !m.is_empty()) {
        command.arg("--model").arg(model);
    }
    // Defence in depth: run outside the project tree so that even if a future
    // change re-enables a tool, the CLI's default working directory is not the
    // user's repo.
    if let Some(temp) = std::env::temp_dir().to_str() {
        command.current_dir(temp);
    }

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to start claude: {e}"))?;

    // Drop stdin after writing so claude sees EOF and starts work.
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open claude stdin".to_string())?;
        stdin
            .write_all(prompt.as_bytes())
            .await
            .map_err(|e| format!("failed to send prompt: {e}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|e| format!("failed to close prompt stream: {e}"))?;
    }

    // Bound the call so a hung CLI can't wedge the chat panel forever.
    let output = tokio::time::timeout(Duration::from_secs(180), child.wait_with_output())
        .await
        .map_err(|_| "claude timed out after 180s".to_string())?
        .map_err(|e| format!("claude failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            format!("claude exited with status {}", output.status)
        } else {
            detail.to_string()
        });
    }

    let answer = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if answer.is_empty() {
        return Err("claude returned an empty response".to_string());
    }
    Ok(answer)
}

#[tauri::command]
async fn open_folder(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let project_path = std::path::PathBuf::from(&path);
    let db_path = project_path.join(".mycelium").join("mycelium.db");
    
    if !db_path.exists() {
        return Err(format!("No mycelium database found in {}", path));
    }
    
    // Open the new database
    let db = Database::open(&db_path).map_err(|e| e.to_string())?;
    
    // Update state
    {
        let mut db_lock = state.db.lock().await;
        *db_lock = db;
    }
    
    // Store the path
    {
        let mut path_lock = state.current_db_path.lock().await;
        *path_lock = Some(project_path.clone());
    }
    
    // Add to recent folders
    if let Ok(app_dir) = app_handle.path().app_config_dir() {
        add_to_recent_folders(&app_dir, path);
    }

    spawn_db_watch(
        app_handle.clone(),
        project_path,
        state.db_watch_generation.clone(),
    );

    // Notify frontend that database changed
    app_handle.emit("database-changed", ()).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
async fn get_recent_folders(
    app_handle: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    let app_dir = app_handle.path().app_config_dir()
        .map_err(|_| "Could not get app config dir")?;
    
    Ok(get_recent_folders_from_disk(&app_dir))
}

// ---------------------------------------------------------------------------
// Helpers: batch-build frontend Task DTOs from core rows (avoids N+1 queries)
// ---------------------------------------------------------------------------

/// Build epic_id -> title and assignee_id -> name lookup maps in two queries,
/// then assemble frontend `Task` DTOs (with `epic_title`/`assignee_name`
/// filled in) and dependency info via a single batched call.
fn build_task_dtos(db: &Database, tasks: Vec<mycelium_core::models::Task>) -> Result<Vec<Task>, String> {
    let epics = db.list_epics().map_err(|e| e.to_string())?;
    let epic_titles: HashMap<i64, String> = epics.into_iter().map(|e| (e.id, e.title)).collect();

    let assignees = db.list_assignees().map_err(|e| e.to_string())?;
    let assignee_names: HashMap<i64, String> =
        assignees.into_iter().map(|a| (a.id, a.name)).collect();

    let ids: Vec<i64> = tasks.iter().map(|t| t.id).collect();
    // blocked_by must only reflect OPEN/IN_PROGRESS blockers (a task blocked
    // solely by a closed task is not blocked). Use the status-filtered variant.
    let deps = db
        .get_active_dependencies_for_tasks(&ids)
        .map_err(|e| e.to_string())?;

    let dtos = tasks
        .into_iter()
        .map(|t| {
            let epic_title = t.epic_id.and_then(|id| epic_titles.get(&id).cloned());
            let assignee_name = t.assignee_id.and_then(|id| assignee_names.get(&id).cloned());
            let (blocked_by, blocks) = deps.get(&t.id).cloned().unwrap_or_default();
            dto::task_from_core(t, epic_title, assignee_name, blocked_by, blocks)
        })
        .collect();

    Ok(dtos)
}

fn build_task_dto(db: &Database, task: mycelium_core::models::Task) -> Result<Task, String> {
    let epic_title = match task.epic_id {
        Some(id) => db
            .get_epic(id)
            .map_err(|e| e.to_string())?
            .map(|e| e.title),
        None => None,
    };
    let assignee_name = match task.assignee_id {
        Some(id) => db
            .get_assignee(id)
            .map_err(|e| e.to_string())?
            .map(|a| a.name),
        None => None,
    };
    // Same active-blocker semantics as the batch path: blocked_by counts only
    // open/in_progress blockers; blocks is the unfiltered reverse edge.
    let deps = db
        .get_active_dependencies_for_tasks(&[task.id])
        .map_err(|e| e.to_string())?;
    let (blocked_by, blocks) = deps.get(&task.id).cloned().unwrap_or_default();
    Ok(dto::task_from_core(
        task,
        epic_title,
        assignee_name,
        blocked_by,
        blocks,
    ))
}

#[tauri::command]
async fn get_dashboard_stats(
    state: tauri::State<'_, AppState>
) -> Result<DashboardStats, String> {
    let db = state.db.lock().await;
    db.get_dashboard_stats().map_err(|e| e.to_string()).map(Into::into)
}

#[tauri::command]
async fn get_tasks(
    state: tauri::State<'_, AppState>,
    filters: TaskFilters,
) -> Result<Vec<Task>, String> {
    let db = state.db.lock().await;
    let core_status: Option<mycelium_core::models::Status> = filters.status.map(Into::into);
    let core_priority: Option<mycelium_core::models::Priority> = filters.priority.map(Into::into);

    // Note: we do NOT pass `blocked` to core list_tasks. Core's blocked filter
    // keys on OPEN-only blockers, but MycUI considers a task blocked if it has
    // any OPEN or IN_PROGRESS blocker. So we fetch unfiltered and apply the
    // blocked filter in the DTO layer, where blocked_by already has the correct
    // (open+in_progress) semantics.
    let tasks = db
        .list_tasks(
            filters.epic_id,
            core_status,
            core_priority,
            filters.assignee_id,
            false,
            filters.overdue,
            filters.tag.as_deref(),
        )
        .map_err(|e| e.to_string())?;

    let mut dtos = build_task_dtos(&db, tasks)?;

    if filters.blocked {
        dtos.retain(|t| !t.blocked_by.is_empty());
    }

    if let Some(search) = filters.search {
        let search_lower = search.to_lowercase();
        dtos.retain(|t| {
            t.title.to_lowercase().contains(&search_lower)
                || t.description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&search_lower))
                    .unwrap_or(false)
        });
    }

    Ok(dtos)
}

#[tauri::command]
async fn get_task(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Option<Task>, String> {
    let db = state.db.lock().await;
    match db.get_task(id).map_err(|e| e.to_string())? {
        Some(task) => Ok(Some(build_task_dto(&db, task)?)),
        None => Ok(None),
    }
}

#[tauri::command]
async fn create_task(
    state: tauri::State<'_, AppState>,
    task: NewTask,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    let due_date = task
        .due_date
        .as_deref()
        .map(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d"))
        .transpose()
        .map_err(|e| e.to_string())?;

    let created = db
        .create_task(
            &task.title,
            task.description.as_deref(),
            task.epic_id,
            task.priority.into(),
            task.assignee_id,
            due_date,
            task.tags.as_deref(),
            None,
            None,
        )
        .map_err(|e| e.to_string())?;

    build_task_dto(&db, created)
}

#[tauri::command]
async fn update_task(
    state: tauri::State<'_, AppState>,
    id: i64,
    updates: TaskUpdate,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;

    let due_date: Option<Option<chrono::NaiveDate>> = match updates.due_date {
        Some(Some(d)) => Some(Some(
            chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|e| e.to_string())?,
        )),
        Some(None) => Some(None),
        None => None,
    };

    let tags: Option<Option<&str>> = updates.tags.as_ref().map(|o| o.as_deref());

    let updated = db
        .update_task(
            id,
            updates.title.as_deref(),
            updates.description.as_deref(),
            updates.status.map(Into::into),
            updates.priority.map(Into::into),
            updates.epic_id,
            updates.assignee_id,
            due_date,
            tags,
            None,
            None,
            None,
        )
        .map_err(|e| e.to_string())?;

    build_task_dto(&db, updated)
}

#[tauri::command]
async fn delete_task(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.delete_task(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_task(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    let updated = db
        .update_task(
            id,
            None,
            None,
            Some(mycelium_core::models::Status::InProgress),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .map_err(|e| e.to_string())?;
    build_task_dto(&db, updated)
}

#[tauri::command]
async fn close_task(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    let blockers = db.get_open_blockers(id).map_err(|e| e.to_string())?;
    if !blockers.is_empty() {
        let blocker_list = blockers
            .iter()
            .map(|task| format!("T{} {}", task.id, task.title))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("Task T{id} is blocked by {blocker_list}"));
    }
    // Forced: the blocker check just ran above, with a UI-specific message.
    let updated = db
        .update_task_forced(
            id,
            None,
            None,
            Some(mycelium_core::models::Status::Closed),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .map_err(|e| e.to_string())?;
    build_task_dto(&db, updated)
}

#[tauri::command]
async fn reopen_task(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    let updated = db
        .update_task(
            id,
            None,
            None,
            Some(mycelium_core::models::Status::Open),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .map_err(|e| e.to_string())?;
    build_task_dto(&db, updated)
}

#[tauri::command]
async fn get_epics(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Epic>, String> {
    let db = state.db.lock().await;
    let summaries = db.list_epics_with_summary().map_err(|e| e.to_string())?;
    Ok(summaries.into_iter().map(Into::into).collect())
}

#[tauri::command]
async fn get_epic(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Option<Epic>, String> {
    let db = state.db.lock().await;
    let summaries = db.list_epics_with_summary().map_err(|e| e.to_string())?;
    Ok(summaries.into_iter().find(|s| s.epic.id == id).map(Into::into))
}

#[tauri::command]
async fn create_epic(
    state: tauri::State<'_, AppState>,
    epic: NewEpic,
) -> Result<Epic, String> {
    let mut db = state.db.lock().await;
    let created = db
        .create_epic(&epic.title, epic.description.as_deref(), None, None)
        .map_err(|e| e.to_string())?;
    let summaries = db.list_epics_with_summary().map_err(|e| e.to_string())?;
    summaries
        .into_iter()
        .find(|s| s.epic.id == created.id)
        .map(Into::into)
        .ok_or_else(|| "Epic not found after creation".to_string())
}

#[tauri::command]
async fn update_epic(
    state: tauri::State<'_, AppState>,
    id: i64,
    updates: EpicUpdate,
) -> Result<Epic, String> {
    let mut db = state.db.lock().await;
    db.update_epic(
        id,
        updates.title.as_deref(),
        updates.description.as_deref(),
        updates.status.map(Into::into),
        None,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;

    let summaries = db.list_epics_with_summary().map_err(|e| e.to_string())?;
    summaries
        .into_iter()
        .find(|s| s.epic.id == id)
        .map(Into::into)
        .ok_or_else(|| "Epic not found after update".to_string())
}

#[tauri::command]
async fn delete_epic(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.delete_epic(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_assignees(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Assignee>, String> {
    let db = state.db.lock().await;
    let stats = db.list_assignees_with_stats().map_err(|e| e.to_string())?;
    Ok(stats.into_iter().map(Into::into).collect())
}

#[tauri::command]
async fn create_assignee(
    state: tauri::State<'_, AppState>,
    assignee: NewAssignee,
) -> Result<Assignee, String> {
    let mut db = state.db.lock().await;
    let created = db
        .create_assignee(
            &assignee.name,
            assignee.email.as_deref(),
            assignee.github_username.as_deref(),
        )
        .map_err(|e| e.to_string())?;
    let stats = db.list_assignees_with_stats().map_err(|e| e.to_string())?;
    stats
        .into_iter()
        .find(|s| s.assignee.id == created.id)
        .map(Into::into)
        .ok_or_else(|| "Assignee not found after creation".to_string())
}

#[tauri::command]
async fn add_dependency(
    state: tauri::State<'_, AppState>,
    task_id: i64,
    depends_on: i64,
) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.add_dependency(task_id, depends_on).map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_dependency(
    state: tauri::State<'_, AppState>,
    task_id: i64,
    depends_on: i64,
) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.remove_dependency(task_id, depends_on).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_dependencies(
    state: tauri::State<'_, AppState>,
    task_id: i64,
) -> Result<DependencyChain, String> {
    let db = state.db.lock().await;
    db.get_all_dependencies(task_id)
        .map_err(|e| e.to_string())
        .map(Into::into)
}

#[tauri::command]
async fn search_tasks(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<Task>, String> {
    let db = state.db.lock().await;
    let tasks = db
        .list_tasks(None, None, None, None, false, false, None)
        .map_err(|e| e.to_string())?;
    let mut dtos = build_task_dtos(&db, tasks)?;

    let search_lower = query.to_lowercase();
    dtos.retain(|t| {
        t.title.to_lowercase().contains(&search_lower)
            || t.description
                .as_ref()
                .map(|d| d.to_lowercase().contains(&search_lower))
                .unwrap_or(false)
    });

    Ok(dtos)
}

#[tauri::command]
async fn get_all_tags(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().await;
    let tasks = db.list_all_tasks().map_err(|e| e.to_string())?;

    let mut all_tags = std::collections::HashSet::new();
    for task in tasks {
        if let Some(tags) = task.tags {
            for tag in tags.split(',') {
                let tag = tag.trim();
                if !tag.is_empty() {
                    all_tags.insert(tag.to_string());
                }
            }
        }
    }

    let mut result: Vec<String> = all_tags.into_iter().collect();
    result.sort();
    Ok(result)
}

#[tauri::command]
async fn list_followups(
    state: tauri::State<'_, AppState>,
    include_closed: Option<bool>,
) -> Result<Vec<Followup>, String> {
    let db = state.db.lock().await;
    let status_filter = if include_closed.unwrap_or(false) {
        None
    } else {
        Some("active")
    };
    db.list_followups(status_filter)
        .map_err(|e| e.to_string())
        .map(|v| v.into_iter().map(Into::into).collect())
}

#[tauri::command]
async fn get_followup(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Option<Followup>, String> {
    let db = state.db.lock().await;
    db.get_followup(id)
        .map_err(|e| e.to_string())
        .map(|o| o.map(Into::into))
}

#[tauri::command]
async fn create_followup(
    state: tauri::State<'_, AppState>,
    followup: NewFollowup,
) -> Result<Followup, String> {
    let mut db = state.db.lock().await;
    db.create_followup(&followup.body, followup.title.as_deref())
        .map_err(|e| e.to_string())
        .map(Into::into)
}

#[tauri::command]
async fn set_followup_status(
    state: tauri::State<'_, AppState>,
    id: i64,
    status: FollowupStatus,
    reason: Option<String>,
) -> Result<Followup, String> {
    let mut db = state.db.lock().await;
    db.update_followup_status(id, status.into(), reason.as_deref())
        .map_err(|e| e.to_string())
        .map(Into::into)
}

#[tauri::command]
async fn update_followup(
    state: tauri::State<'_, AppState>,
    id: i64,
    body: Option<String>,
    title: Option<Option<String>>,
) -> Result<Followup, String> {
    let mut db = state.db.lock().await;
    // mycui's `title: Option<Option<String>>` (outer: "was a title value passed
    // at all", inner: "clear it vs set it") maps onto core's two-parameter
    // `(title: Option<&str>, clear_title: bool)` as follows:
    //   outer None       -> (None, false)       leave title untouched
    //   outer Some(None) -> (None, true)         clear title
    //   outer Some(Some(s)) -> (Some(&s), false) set title to s
    let (core_title, clear_title): (Option<&str>, bool) = match &title {
        None => (None, false),
        Some(None) => (None, true),
        Some(Some(s)) => (Some(s.as_str()), false),
    };

    db.update_followup_body(id, body.as_deref(), core_title, clear_title)
        .map_err(|e| e.to_string())
        .map(Into::into)
}

#[tauri::command]
async fn append_followup(
    state: tauri::State<'_, AppState>,
    id: i64,
    text: String,
) -> Result<Followup, String> {
    let mut db = state.db.lock().await;
    // core has no append primitive; replicate mycui's previous behavior here:
    // append a "[timestamp] text" block to the existing body, matching the
    // CLI's timestamp format so entries stay consistent across tools.
    let existing = db
        .get_followup(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Follow-up F{id} not found"))?;

    let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M %z");
    let new_body = if existing.body.trim().is_empty() {
        format!("[{}] {}", stamp, text)
    } else {
        format!("{}\n\n[{}] {}", existing.body.trim_end(), stamp, text)
    };

    db.update_followup_body(id, Some(&new_body), None, false)
        .map_err(|e| e.to_string())
        .map(Into::into)
}

#[tauri::command]
async fn delete_followup(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let mut db = state.db.lock().await;
    db.delete_followup(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn count_followups(
    state: tauri::State<'_, AppState>,
) -> Result<FollowupCounts, String> {
    let db = state.db.lock().await;
    db.count_followups().map_err(|e| e.to_string()).map(Into::into)
}

fn main() {
    // Find the mycelium database
    let db_path = find_mycelium_db();
    
    let (db, current_db_path) = match db_path {
        Some(ref path) => {
            println!("Found mycelium database at: {:?}", path);
            let project_path = path.parent()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf());
            (Database::open(path).expect("Failed to open database"), project_path)
        }
        None => {
            println!("No mycelium database found, creating in-memory database");
            (Database::open_in_memory().expect("Failed to create in-memory database"), None)
        }
    };

    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        current_db_path: Arc::new(Mutex::new(current_db_path)),
        db_watch_generation: Arc::new(AtomicU64::new(0)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, _shortcut, event| {
                if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Some(window) = app.get_webview_window("main") {
                        if !window.is_visible().unwrap_or(false) {
                            let _ = window.show();
                        }
                        let _ = window.set_focus();
                        let _ = window.emit("quick-add", ());
                    }
                }
            })
            .build())
        .manage(state)
        .setup(|app| {
            if let Some(project_path) = app.state::<AppState>().current_db_path
                .blocking_lock()
                .as_ref()
                .cloned()
            {
                let watch_generation = app.state::<AppState>().db_watch_generation.clone();
                spawn_db_watch(app.handle().clone(), project_path, watch_generation);
            }

            // Create tray menu
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quick_add_i = MenuItem::with_id(app, "quick_add", "Quick Add", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quick_add_i, &quit_i])?;

            // Build tray icon
            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quick_add" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.emit("quick-add", ());
                        }
                    }
                    "quit" => {
                        std::process::exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // Register global shortcut (Cmd/Ctrl+Shift+T)
            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyT);
            app.global_shortcut().register(shortcut).map_err(|e| {
                println!("Failed to register shortcut: {}", e);
            }).ok();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_folder_dialog,
            get_current_db_path,
            open_folder,
            get_recent_folders,
            get_dashboard_stats,
            get_tasks,
            get_task,
            create_task,
            update_task,
            delete_task,
            start_task,
            close_task,
            reopen_task,
            get_epics,
            get_epic,
            create_epic,
            update_epic,
            delete_epic,
            get_assignees,
            create_assignee,
            add_dependency,
            remove_dependency,
            get_dependencies,
            search_tasks,
            get_all_tags,
            list_followups,
            get_followup,
            create_followup,
            set_followup_status,
            update_followup,
            append_followup,
            delete_followup,
            count_followups,
            claude_available,
            ask_claude,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
