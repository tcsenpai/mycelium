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

mod db;
mod models;

use db::Database;
use models::*;

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
        std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
            .unwrap_or_default()
    } else {
        AppConfig::default()
    };
    
    // Remove if already exists
    config.recent_folders.retain(|f| f != &path);
    
    // Add to front
    config.recent_folders.insert(0, path);
    
    // Limit to max
    config.recent_folders.truncate(MAX_RECENT_FOLDERS);
    
    let _ = std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap_or_default());
}

fn get_recent_folders_from_disk(app_dir: &std::path::Path) -> Vec<String> {
    let config_path: std::path::PathBuf = app_dir.join("config.json");
    
    if !config_path.exists() {
        return Vec::new();
    }
    
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
    let config: AppConfig = serde_json::from_str(&content).unwrap_or_default();
    
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

#[tauri::command]
async fn get_dashboard_stats(
    state: tauri::State<'_, AppState>
) -> Result<DashboardStats, String> {
    let db = state.db.lock().await;
    db.get_dashboard_stats().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_tasks(
    state: tauri::State<'_, AppState>,
    filters: TaskFilters,
) -> Result<Vec<Task>, String> {
    let db = state.db.lock().await;
    db.get_tasks(filters).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_task(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Option<Task>, String> {
    let db = state.db.lock().await;
    db.get_task(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_task(
    state: tauri::State<'_, AppState>,
    task: NewTask,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    db.create_task(task).map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_task(
    state: tauri::State<'_, AppState>,
    id: i64,
    updates: TaskUpdate,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    db.update_task(id, updates).map_err(|e| e.to_string())
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
async fn close_task(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    let blockers = db.get_open_blockers(id).map_err(|e| e.to_string())?;
    if !blockers.is_empty() {
        let blocker_list = blockers
            .iter()
            .map(|task| format!("#{} {}", task.id, task.title))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("Task #{id} is blocked by {blocker_list}"));
    }
    db.close_task(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn reopen_task(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    db.reopen_task(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_epics(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Epic>, String> {
    let db = state.db.lock().await;
    db.get_epics().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_epic(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<Option<Epic>, String> {
    let db = state.db.lock().await;
    db.get_epic(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_epic(
    state: tauri::State<'_, AppState>,
    epic: NewEpic,
) -> Result<Epic, String> {
    let mut db = state.db.lock().await;
    db.create_epic(epic).map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_epic(
    state: tauri::State<'_, AppState>,
    id: i64,
    updates: EpicUpdate,
) -> Result<Epic, String> {
    let mut db = state.db.lock().await;
    db.update_epic(id, updates).map_err(|e| e.to_string())
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
    db.get_assignees().map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_assignee(
    state: tauri::State<'_, AppState>,
    assignee: NewAssignee,
) -> Result<Assignee, String> {
    let mut db = state.db.lock().await;
    db.create_assignee(assignee).map_err(|e| e.to_string())
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
    db.get_dependencies(task_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_tasks(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<Task>, String> {
    let db = state.db.lock().await;
    db.search_tasks(&query).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_all_tags(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().await;
    db.get_all_tags().map_err(|e| e.to_string())
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
