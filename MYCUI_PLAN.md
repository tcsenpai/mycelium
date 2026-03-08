# MycUI - Native Desktop Dashboard for Mycelium

## Overview

A native desktop application providing a visual dashboard for managing Mycelium tasks, with real-time updates, drag-and-drop interfaces, and rich visualizations.

## Technology Comparison: Tauri vs Electron

### Tauri (Recommended ✅)

**Pros:**
- **Performance**: Rust backend, ~600KB binary (vs 100MB+ Electron)
- **Security**: Memory-safe Rust, minimal attack surface
- **Resource Usage**: ~50MB RAM (vs 200-500MB Electron)
- **Native Feel**: Uses OS webview (WebKit on macOS, WebView2 on Windows)
- **Fast Startup**: ~1s cold start (vs 5-10s Electron)
- **Single Binary**: Easy distribution, auto-updater built-in
- **Rust Ecosystem**: Reuse mycelium core logic directly

**Cons:**
- **WebView Compatibility**: Slight differences across platforms
- **Learning Curve**: Need Rust knowledge for backend
- **Smaller Community**: Less Stack Overflow help vs Electron
- **Limited APIs**: Some OS features require native plugins

### Electron

**Pros:**
- **Mature Ecosystem**: Huge community, extensive docs
- **Consistent Experience**: Same Chromium everywhere
- **Easy Development**: Just Node.js + web tech
- **Rich APIs**: Extensive OS integration out of box
- **VS Code/Slack**: Battle-tested by major apps

**Cons:**
- **Bloated**: 100-200MB downloads, 300-500MB RAM
- **Slow Startup**: 5-10 seconds typical
- **Security Concerns**: Large attack surface (Chromium + Node)
- **Battery Drain**: Higher resource usage
- **Separate Backend**: Need to spawn myc binary or use IPC

## Decision: Use Tauri

**Rationale:**
1. **Performance matters** for a task manager that runs in background
2. **Rust synergy** - can share code with mycelium CLI
3. **Distribution** - single small binary is professional
4. **Modern approach** - Tauri is gaining rapid adoption
5. **Beads UI** was web-based; native app differentiates us

## Architecture

```
mycui/
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── main.rs      # Entry point
│   │   ├── db.rs        # SQLite access (shared with myc)
│   │   ├── commands.rs  # Tauri command handlers
│   │   └── state.rs     # App state management
│   └── Cargo.toml
├── src/                 # Frontend (React/Vue/Svelte)
│   ├── components/      # UI components
│   ├── views/           # Page views
│   ├── stores/          # State management
│   └── lib/
│       └── api.ts       # Rust bridge API
├── public/
└── package.json
```

## Core Features

### Phase 1: Dashboard (MVP)
- [ ] **Overview Cards**: Total tasks, completed, overdue, blocked
- [ ] **Task List**: Sortable, filterable table view
- [ ] **Quick Actions**: Create, edit, close tasks inline
- [ ] **Search**: Full-text search across tasks
- [ ] **Dark/Light Mode**: Native theme switching

### Phase 2: Visual Management
- [ ] **Kanban Board**: Drag-and-drop between statuses
- [ ] **Epic View**: Grouped by epic, progress bars
- [ ] **Dependency Graph**: Visual task relationships
- [ ] **Calendar View**: Tasks by due date
- [ ] **Timeline/Gantt**: Project timeline view

### Phase 3: Advanced Features
- [ ] **Real-time Sync**: Watch file changes, auto-refresh
- [ ] **Notifications**: Due date reminders, blocker alerts
- [ ] **Batch Operations**: Multi-select, bulk edit
- [ ] **Import/Export**: JSON/CSV with drag-drop
- [ ] **Offline Mode**: Work without sync, queue changes

### Phase 4: Integration
- [ ] **GitHub Integration**: View linked issues/PRs
- [ ] **System Tray**: Quick add from anywhere
- [ ] **Global Hotkey**: Cmd/Ctrl+Shift+T to quick add
- [ ] **Menu Bar Widget**: macOS menu bar / Windows system tray
- [ ] **CLI Bridge**: Use myc commands from UI

## UI Design

### Layout
```
┌─────────────────────────────────────────────────────────┐
│  Sidebar    │  Main Content                             │
│             │                                           │
│  📊 Dashboard│  ┌─────────────────────────────────────┐  │
│  📋 Tasks   │  │  Overview Cards                     │  │
│  📁 Epics   │  │  [Open: 12] [Done: 45] [Overdue: 2]│  │
│  🏷️ Tags    │  └─────────────────────────────────────┘  │
│  👥 Team    │                                           │
│             │  ┌─────────────────────────────────────┐  │
│  ───────────│  │  Task List / Kanban / Calendar      │  │
│             │  │  [Filter Bar]                       │  │
│  ⚙️ Settings│  │                                     │  │
│             │  │  ☐ Task 1                  [drag]   │  │
│             │  │  ☐ Task 2                  [handle] │  │
│             │  │  ☑ Task 3 (completed)              │  │
│             │  │                                     │  │
│             │  └─────────────────────────────────────┘  │
│             │                                           │
└─────────────────────────────────────────────────────────┘
```

### Components

1. **TaskCard**: Compact task display with priority colors
2. **EpicRow**: Collapsible epic with progress bar
3. **FilterBar**: Quick filters (status, priority, assignee, tags)
4. **QuickAdd**: Floating button + modal for fast task creation
5. **DependencyGraph**: D3/Vis.js network visualization
6. **Calendar**: FullCalendar or similar integration

## Technical Implementation

### Backend (Tauri/Rust)

```rust
// src-tauri/src/commands.rs
#[tauri::command]
async fn get_tasks(
    state: tauri::State<'_, AppState>,
    filters: TaskFilters
) -> Result<Vec<Task>, String> {
    let db = state.db.lock().await;
    db.list_tasks_filtered(filters)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn create_task(
    state: tauri::State<'_, AppState>,
    task: NewTask
) -> Result<Task, String> {
    let mut db = state.db.lock().await;
    db.create_task(&task)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn watch_db_changes(
    state: tauri::State<'_, AppState>,
    window: tauri::Window
) -> Result<(), String> {
    // Watch .mycelium/mycelium.db for changes
    // Emit events to frontend
}
```

### Frontend (React + TypeScript)

```typescript
// src/lib/api.ts
import { invoke } from '@tauri-apps/api/tauri';

export async function getTasks(filters: TaskFilters): Promise<Task[]> {
  return invoke('get_tasks', { filters });
}

export async function createTask(task: NewTask): Promise<Task> {
  return invoke('create_task', { task });
}

// Auto-refresh on DB changes
import { listen } from '@tauri-apps/api/event';

listen('db-changed', () => {
  queryClient.invalidateQueries(['tasks']);
});
```

### State Management

Use **Zustand** or **Jotai** for lightweight state:

```typescript
// src/stores/taskStore.ts
import { create } from 'zustand';

interface TaskStore {
  tasks: Task[];
  filters: Filters;
  viewMode: 'list' | 'kanban' | 'calendar';
  setTasks: (tasks: Task[]) => void;
  setFilters: (filters: Filters) => void;
  createTask: (task: NewTask) => Promise<void>;
}

export const useTaskStore = create<TaskStore>((set, get) => ({
  tasks: [],
  filters: {},
  viewMode: 'list',
  setTasks: (tasks) => set({ tasks }),
  setFilters: (filters) => set({ filters }),
  createTask: async (task) => {
    const newTask = await api.createTask(task);
    set({ tasks: [...get().tasks, newTask] });
  },
}));
```

## Development Roadmap

### Phase 1: Foundation (2 weeks)
- [ ] Set up Tauri project structure
- [ ] Configure Rust backend with shared DB layer
- [ ] Set up React + TypeScript + Tailwind
- [ ] Implement basic commands (get_tasks, create_task)
- [ ] Build sidebar navigation

### Phase 2: Core Views (2 weeks)
- [ ] Dashboard with overview cards
- [ ] Task list with sorting/filtering
- [ ] Epic list view
- [ ] Quick add modal
- [ ] Dark/light theme

### Phase 3: Visual Features (2 weeks)
- [ ] Kanban board with dnd-kit
- [ ] Calendar view
- [ ] Dependency visualization
- [ ] Search functionality

### Phase 4: Polish (2 weeks)
- [ ] System tray integration
- [ ] Global hotkeys
- [ ] Auto-updater
- [ ] Packaging (DMG, MSI, AppImage)

## Package Distribution

### macOS
- `.dmg` with drag-to-Applications
- Homebrew: `brew install --cask mycui`
- Mac App Store (optional)

### Windows
- `.msi` installer
- Microsoft Store (optional)
- winget: `winget install Mycelium.MycUI`

### Linux
- `.AppImage` (universal)
- `.deb` for Debian/Ubuntu
- `.rpm` for Fedora
- AUR package for Arch

## Pricing/Monetization (Optional)

**Free (Open Source)**
- All core features
- Community support

**Pro (Paid)**
- Cloud sync
- Team collaboration
- Advanced analytics
- Priority support

## Success Metrics

1. **Performance**: <100MB RAM, <2s startup
2. **UX**: Task creation in <5 seconds
3. **Adoption**: 1000+ GitHub stars in 6 months
4. **Stability**: <1% crash rate

## Resources

- **Tauri Docs**: https://tauri.app/
- **Comparison**: https://tauri.app/about/comparisons/
- **Example Apps**: https://github.com/tauri-apps/awesome-tauri
- **UI Inspiration**: Linear, Height, GitHub Projects
