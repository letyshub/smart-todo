# Smart Todo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Smart Todo — a cross-platform desktop app (Windows/Linux/macOS) with multiple todo lists, rich task metadata, Markdown support, a time tracker, and a priority dashboard.

**Architecture:** Tauri 2 (Rust backend) + React 18 (TypeScript frontend) + SQLite via rusqlite. All data access goes through typed Tauri commands; the frontend never touches the DB directly. Timer state lives in a Rust `Mutex<HashMap<i64, TimerEntry>>` for accurate wall-clock tracking.

**Tech Stack:** Tauri 2, React 18, TypeScript 5, Vite 5, Tailwind CSS 3, Zustand 4, react-markdown 9, remark-gfm 4, rusqlite 0.31 (bundled), chrono 0.4, Vitest 1, @testing-library/react 14, Playwright

---

## File Map

| File | Responsibility |
|---|---|
| `src-tauri/src/main.rs` | Tauri entry point, register commands + state |
| `src-tauri/src/db.rs` | SQLite connection, migrations, `DbState` |
| `src-tauri/src/commands/mod.rs` | Re-export command modules |
| `src-tauri/src/commands/lists.rs` | `get_lists`, `create_list`, `update_list`, `delete_list` |
| `src-tauri/src/commands/tasks.rs` | `get_tasks`, `create_task`, `update_task`, `delete_task`, `get_dashboard_tasks`, `set_task_tags`, `get_all_tags` |
| `src-tauri/src/commands/timer.rs` | `start_timer`, `stop_timer`, `get_active_timers`, `get_timer_sessions`, `TimerState` |
| `src-tauri/src/commands/settings.rs` | `get_settings`, `set_setting`, `change_data_dir` |
| `src/types.ts` | TypeScript types mirroring Rust structs |
| `src/lib/tauri.ts` | Typed `invoke()` wrappers for all commands |
| `src/lib/timeUtils.ts` | Format seconds → `Xh Ym`, `MM:SS` |
| `src/lib/dateUtils.ts` | Date helpers: isOverdue, isToday, isUpcoming |
| `src/store/listsStore.ts` | Zustand store for lists |
| `src/store/tasksStore.ts` | Zustand store for tasks |
| `src/store/timerStore.ts` | Zustand store for timer (polls every 1s) |
| `src/store/settingsStore.ts` | Zustand store for settings |
| `src/App.tsx` | Root layout: Sidebar + main area routing |
| `src/components/Sidebar.tsx` | List nav, tags section, new-list button |
| `src/components/TaskCard.tsx` | Task row: title, tags, due, priority, timer |
| `src/components/TaskEditor.tsx` | Right-panel drawer for editing a task |
| `src/components/TimerWidget.tsx` | Start/stop button + live counter |
| `src/components/MarkdownRenderer.tsx` | Safe react-markdown renderer |
| `src/components/TagInput.tsx` | Autocomplete tag input |
| `src/pages/Dashboard.tsx` | Overdue / High Priority / Upcoming sections |
| `src/pages/ListDetail.tsx` | Task list for one list |
| `src/pages/Settings.tsx` | Data dir + theme settings |

---

## Task 1: Initialize Tauri 2 + React TypeScript project ✅ DONE

## Task 2: Configure Tailwind CSS + Vite ✅ DONE

---

## Task 3: Database module — connection + migrations

**Files:**
- Create: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/main.rs`
- Create: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: Write `src-tauri/src/db.rs`**

```rust
use rusqlite::{Connection, Result};
use std::sync::Mutex;

pub struct DbState(pub Mutex<Connection>);

pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS lists (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            title      TEXT NOT NULL,
            color      TEXT,
            position   INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            list_id      INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
            title        TEXT NOT NULL,
            description  TEXT,
            priority     TEXT NOT NULL DEFAULT 'normal' CHECK(priority IN ('normal','high')),
            due_date     TEXT,
            completed    INTEGER NOT NULL DEFAULT 0,
            completed_at TEXT,
            position     INTEGER NOT NULL DEFAULT 0,
            created_at   TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tags (
            id    INTEGER PRIMARY KEY AUTOINCREMENT,
            name  TEXT NOT NULL UNIQUE,
            color TEXT
        );

        CREATE TABLE IF NOT EXISTS task_tags (
            task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (task_id, tag_id)
        );

        CREATE TABLE IF NOT EXISTS timer_sessions (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id          INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            started_at       TEXT NOT NULL,
            stopped_at       TEXT,
            duration_seconds INTEGER
        );

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ")
}

#[cfg(test)]
pub fn open_in_memory() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    migrate(&conn).unwrap();
    conn
}
```

- [ ] **Step 2: Write `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod commands;

use db::DbState;
use commands::timer::TimerState;
use std::sync::Mutex;
use std::collections::HashMap;

fn main() {
    let app_data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("smart-todo");
    std::fs::create_dir_all(&app_data_dir).unwrap();
    let db_path = app_data_dir.join("data.db").to_string_lossy().to_string();

    let conn = db::open(&db_path).expect("failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DbState(Mutex::new(conn)))
        .manage(TimerState(Mutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![
            commands::lists::get_lists,
            commands::lists::create_list,
            commands::lists::update_list,
            commands::lists::delete_list,
            commands::tasks::get_tasks,
            commands::tasks::create_task,
            commands::tasks::update_task,
            commands::tasks::delete_task,
            commands::tasks::get_dashboard_tasks,
            commands::tasks::set_task_tags,
            commands::tasks::get_all_tags,
            commands::timer::start_timer,
            commands::timer::stop_timer,
            commands::timer::get_active_timers,
            commands::timer::get_timer_sessions,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::change_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Create `src-tauri/src/commands/mod.rs`**

```rust
pub mod lists;
pub mod tasks;
pub mod timer;
pub mod settings;
```

- [ ] **Step 4: Create stub files for the other command modules** so `main.rs` compiles

Create `src-tauri/src/commands/lists.rs`:
```rust
use crate::db::DbState;
use tauri::State;

#[tauri::command]
pub fn get_lists(_state: State<DbState>) -> Result<Vec<()>, String> { Ok(vec![]) }
#[tauri::command]
pub fn create_list(_state: State<DbState>, _title: String) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn update_list(_state: State<DbState>, _id: i64) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn delete_list(_state: State<DbState>, _id: i64) -> Result<(), String> { Ok(()) }
```

Create `src-tauri/src/commands/tasks.rs`:
```rust
use crate::db::DbState;
use tauri::State;

#[tauri::command]
pub fn get_tasks(_state: State<DbState>, _list_id: i64) -> Result<Vec<()>, String> { Ok(vec![]) }
#[tauri::command]
pub fn create_task(_state: State<DbState>, _list_id: i64, _title: String) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn update_task(_state: State<DbState>, _id: i64) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn delete_task(_state: State<DbState>, _id: i64) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn get_dashboard_tasks(_state: State<DbState>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn set_task_tags(_state: State<DbState>, _task_id: i64, _tag_names: Vec<String>) -> Result<Vec<()>, String> { Ok(vec![]) }
#[tauri::command]
pub fn get_all_tags(_state: State<DbState>) -> Result<Vec<()>, String> { Ok(vec![]) }
```

Create `src-tauri/src/commands/timer.rs`:
```rust
use crate::db::DbState;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

pub struct TimerEntry {
    pub start_instant: Instant,
    pub started_at: String,
}
pub struct TimerState(pub Mutex<HashMap<i64, TimerEntry>>);

#[tauri::command]
pub fn start_timer(_task_id: i64, _timer_state: State<TimerState>, _db_state: State<DbState>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn stop_timer(_task_id: i64, _timer_state: State<TimerState>, _db_state: State<DbState>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn get_active_timers(_timer_state: State<TimerState>) -> Result<Vec<()>, String> { Ok(vec![]) }
#[tauri::command]
pub fn get_timer_sessions(_task_id: i64, _state: State<DbState>) -> Result<Vec<()>, String> { Ok(vec![]) }
```

Create `src-tauri/src/commands/settings.rs`:
```rust
use crate::db::DbState;
use tauri::State;

#[tauri::command]
pub fn get_settings(_state: State<DbState>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn set_setting(_state: State<DbState>, _key: String, _value: String) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn change_data_dir(_state: State<DbState>, _new_path: String) -> Result<(), String> { Ok(()) }
```

- [ ] **Step 5: Also update `src-tauri/src/lib.rs`** to remove the old run() function body and replace with a minimal one (since main.rs now handles setup):

Check what `lib.rs` currently contains. If it has a `run()` function that conflicts with `main.rs`, replace its body with just:
```rust
// pub-run entry used by mobile targets; desktop uses main.rs directly
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Desktop entry is in main.rs
}
```

If `lib.rs` already just has plugins and run(), leave it as-is and ensure `main.rs` calls `app_lib::run()` instead of the inline setup. **Check the current state first before making changes.**

Actually — the cleanest approach for Tauri 2 is to keep all setup in `lib.rs` and have `main.rs` just call `app_lib::run()`. So instead of the `main.rs` above, put all the builder code in `lib.rs`:

**`src-tauri/src/lib.rs`:**
```rust
mod db;
mod commands;

use db::DbState;
use commands::timer::TimerState;
use std::sync::Mutex;
use std::collections::HashMap;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("smart-todo");
    std::fs::create_dir_all(&app_data_dir).unwrap();
    let db_path = app_data_dir.join("data.db").to_string_lossy().to_string();
    let conn = db::open(&db_path).expect("failed to open database");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DbState(Mutex::new(conn)))
        .manage(TimerState(Mutex::new(HashMap::new())))
        .invoke_handler(tauri::generate_handler![
            commands::lists::get_lists,
            commands::lists::create_list,
            commands::lists::update_list,
            commands::lists::delete_list,
            commands::tasks::get_tasks,
            commands::tasks::create_task,
            commands::tasks::update_task,
            commands::tasks::delete_task,
            commands::tasks::get_dashboard_tasks,
            commands::tasks::set_task_tags,
            commands::tasks::get_all_tags,
            commands::timer::start_timer,
            commands::timer::stop_timer,
            commands::timer::get_active_timers,
            commands::timer::get_timer_sessions,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::settings::change_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**`src-tauri/src/main.rs`** stays as the scaffold generated it:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    app_lib::run();
}
```

- [ ] **Step 6: Run `cargo check` to verify it compiles**

```powershell
cd src-tauri && cargo check 2>&1
```
Expected: no errors (warnings OK).

- [ ] **Step 7: Write migration test in `db.rs`**

Add to `src-tauri/src/db.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_create_all_tables() {
        let conn = open_in_memory();
        let tables: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
            ).unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert!(tables.contains(&"lists".to_string()));
        assert!(tables.contains(&"tasks".to_string()));
        assert!(tables.contains(&"tags".to_string()));
        assert!(tables.contains(&"task_tags".to_string()));
        assert!(tables.contains(&"timer_sessions".to_string()));
        assert!(tables.contains(&"settings".to_string()));
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = open_in_memory();
        let fk_enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1);
    }
}
```

- [ ] **Step 8: Run tests**

```powershell
cd src-tauri && cargo test db:: -- --nocapture
```
Expected: 2 tests pass.

- [ ] **Step 9: Commit**

```powershell
git add .
git commit -m "feat: add SQLite database module with schema migrations and stub commands"
```

---

## Task 4: Lists CRUD Rust commands

**Files:**
- Modify: `src-tauri/src/commands/lists.rs`

- [ ] **Step 1: Replace `src-tauri/src/commands/lists.rs` with full implementation**

```rust
use crate::db::DbState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct List {
    pub id: i64,
    pub title: String,
    pub color: Option<String>,
    pub position: i64,
    pub created_at: String,
}

#[tauri::command]
pub fn get_lists(state: State<DbState>) -> Result<Vec<List>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, title, color, position, created_at FROM lists ORDER BY position")
        .map_err(|e| e.to_string())?;
    let lists = stmt
        .query_map([], |row| {
            Ok(List {
                id: row.get(0)?,
                title: row.get(1)?,
                color: row.get(2)?,
                position: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(lists)
}

#[tauri::command]
pub fn create_list(
    state: State<DbState>,
    title: String,
    color: Option<String>,
) -> Result<List, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let position: i64 = conn
        .query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM lists", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO lists (title, color, position) VALUES (?1, ?2, ?3)",
        rusqlite::params![title, color, position],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id, title, color, position, created_at FROM lists WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(List {
                id: row.get(0)?,
                title: row.get(1)?,
                color: row.get(2)?,
                position: row.get(3)?,
                created_at: row.get(4)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_list(
    state: State<DbState>,
    id: i64,
    title: Option<String>,
    color: Option<String>,
    position: Option<i64>,
) -> Result<List, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(t) = title {
        conn.execute("UPDATE lists SET title = ?1 WHERE id = ?2", rusqlite::params![t, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(c) = color {
        conn.execute("UPDATE lists SET color = ?1 WHERE id = ?2", rusqlite::params![c, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(p) = position {
        conn.execute("UPDATE lists SET position = ?1 WHERE id = ?2", rusqlite::params![p, id])
            .map_err(|e| e.to_string())?;
    }
    conn.query_row(
        "SELECT id, title, color, position, created_at FROM lists WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(List {
                id: row.get(0)?,
                title: row.get(1)?,
                color: row.get(2)?,
                position: row.get(3)?,
                created_at: row.get(4)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_list(state: State<DbState>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM lists WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::open_in_memory;

    #[test]
    fn test_create_and_get_list() {
        let conn = open_in_memory();
        conn.execute(
            "INSERT INTO lists (title, color, position) VALUES (?1, ?2, ?3)",
            rusqlite::params!["Work", "#3b82f6", 0],
        ).unwrap();
        let id: i64 = conn.last_insert_rowid();
        let title: String = conn.query_row(
            "SELECT title FROM lists WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(title, "Work");
    }

    #[test]
    fn test_delete_list_cascades_tasks() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('Test', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, title, position) VALUES (?1, 'task', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        conn.execute("DELETE FROM lists WHERE id = ?1", rusqlite::params![list_id]).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE list_id = ?1",
            rusqlite::params![list_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 2: Run tests**

```powershell
cd src-tauri && cargo test commands::lists -- --nocapture
```
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```powershell
git add src-tauri/src/commands/lists.rs
git commit -m "feat: implement lists CRUD Rust commands"
```

---

## Task 5: Tasks CRUD Rust commands

**Files:**
- Modify: `src-tauri/src/commands/tasks.rs`

- [ ] **Step 1: Replace `src-tauri/src/commands/tasks.rs` with full implementation**

```rust
use crate::db::DbState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub list_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub due_date: Option<String>,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<Tag>,
    pub total_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardData {
    pub overdue: Vec<Task>,
    pub high_priority: Vec<Task>,
    pub upcoming: Vec<Task>,
}

fn fetch_tags_for_task(conn: &rusqlite::Connection, task_id: i64) -> Vec<Tag> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.color FROM tags t
             JOIN task_tags tt ON tt.tag_id = t.id
             WHERE tt.task_id = ?1",
        )
        .unwrap();
    stmt.query_map(rusqlite::params![task_id], |r| {
        Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

fn fetch_total_seconds(conn: &rusqlite::Connection, task_id: i64) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM timer_sessions WHERE task_id = ?1",
        rusqlite::params![task_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn row_to_task(row: &rusqlite::Row, conn: &rusqlite::Connection) -> rusqlite::Result<Task> {
    let id: i64 = row.get(0)?;
    let tags = fetch_tags_for_task(conn, id);
    let total_seconds = fetch_total_seconds(conn, id);
    Ok(Task {
        id,
        list_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        priority: row.get(4)?,
        due_date: row.get(5)?,
        completed: row.get::<_, i64>(6)? != 0,
        completed_at: row.get(7)?,
        position: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        tags,
        total_seconds,
    })
}

#[tauri::command]
pub fn get_tasks(state: State<DbState>, list_id: i64) -> Result<Vec<Task>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                    position,created_at,updated_at
             FROM tasks WHERE list_id=?1 ORDER BY completed ASC, position ASC",
        )
        .map_err(|e| e.to_string())?;
    let tasks: Vec<Task> = stmt
        .query_map(rusqlite::params![list_id], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}

#[tauri::command]
pub fn create_task(
    state: State<DbState>,
    list_id: i64,
    title: String,
    priority: Option<String>,
    due_date: Option<String>,
    description: Option<String>,
) -> Result<Task, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE list_id = ?1",
            rusqlite::params![list_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let priority = priority.unwrap_or_else(|| "normal".to_string());
    conn.execute(
        "INSERT INTO tasks (list_id,title,description,priority,due_date,position)
         VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![list_id, title, description, priority, due_date, position],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                position,created_at,updated_at FROM tasks WHERE id=?1",
        rusqlite::params![id],
        |row| row_to_task(row, &conn),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_task(
    state: State<DbState>,
    id: i64,
    title: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    due_date: Option<String>,
    completed: Option<bool>,
    position: Option<i64>,
) -> Result<Task, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(v) = title {
        conn.execute("UPDATE tasks SET title=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = description {
        conn.execute("UPDATE tasks SET description=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = priority {
        conn.execute("UPDATE tasks SET priority=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = due_date {
        let val: Option<String> = if v.is_empty() { None } else { Some(v) };
        conn.execute("UPDATE tasks SET due_date=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![val, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = completed {
        conn.execute(
            "UPDATE tasks SET completed=?1,completed_at=CASE WHEN ?1=1 THEN datetime('now') ELSE NULL END,
             updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v as i64, id],
        ).map_err(|e| e.to_string())?;
    }
    if let Some(v) = position {
        conn.execute("UPDATE tasks SET position=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    conn.query_row(
        "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                position,created_at,updated_at FROM tasks WHERE id=?1",
        rusqlite::params![id],
        |row| row_to_task(row, &conn),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_task(state: State<DbState>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM tasks WHERE id=?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_task_tags(
    state: State<DbState>,
    task_id: i64,
    tag_names: Vec<String>,
) -> Result<Vec<Tag>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM task_tags WHERE task_id=?1", rusqlite::params![task_id])
        .map_err(|e| e.to_string())?;
    for name in &tag_names {
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", rusqlite::params![name])
            .map_err(|e| e.to_string())?;
        let tag_id: i64 = conn
            .query_row("SELECT id FROM tags WHERE name=?1", rusqlite::params![name], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR IGNORE INTO task_tags (task_id, tag_id) VALUES (?1,?2)",
            rusqlite::params![task_id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(fetch_tags_for_task(&conn, task_id))
}

#[tauri::command]
pub fn get_all_tags(state: State<DbState>) -> Result<Vec<Tag>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, color FROM tags ORDER BY name")
        .map_err(|e| e.to_string())?;
    let tags = stmt
        .query_map([], |r| Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? }))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tags)
}

#[tauri::command]
pub fn get_dashboard_tasks(state: State<DbState>) -> Result<DashboardData, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let upcoming_limit = chrono::Local::now()
        .checked_add_days(chrono::Days::new(7))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();

    let base = "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                        position,created_at,updated_at FROM tasks WHERE completed=0";

    let mut stmt = conn.prepare(
        &format!("{base} AND due_date IS NOT NULL AND due_date <= ?1 ORDER BY due_date ASC, priority DESC")
    ).map_err(|e| e.to_string())?;
    let overdue: Vec<Task> = stmt
        .query_map(rusqlite::params![today], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    let overdue_ids: Vec<i64> = overdue.iter().map(|t| t.id).collect();

    let mut stmt2 = conn.prepare(
        &format!("{base} AND priority='high' ORDER BY due_date ASC")
    ).map_err(|e| e.to_string())?;
    let high_priority: Vec<Task> = stmt2
        .query_map([], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .filter(|t| !overdue_ids.contains(&t.id))
        .collect();

    let mut exclude_ids = overdue_ids.clone();
    exclude_ids.extend(high_priority.iter().map(|t| t.id));

    let mut stmt3 = conn.prepare(
        &format!("{base} AND due_date IS NOT NULL AND due_date > ?1 AND due_date <= ?2 ORDER BY due_date ASC")
    ).map_err(|e| e.to_string())?;
    let upcoming: Vec<Task> = stmt3
        .query_map(rusqlite::params![today, upcoming_limit], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .filter(|t| !exclude_ids.contains(&t.id))
        .collect();

    Ok(DashboardData { overdue, high_priority, upcoming })
}

#[cfg(test)]
mod tests {
    use crate::db::open_in_memory;

    fn setup() -> (rusqlite::Connection, i64) {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('Test', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        (conn, list_id)
    }

    #[test]
    fn test_create_task() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, ?2, 'normal', 0)",
            rusqlite::params![list_id, "My Task"],
        ).unwrap();
        let title: String = conn.query_row(
            "SELECT title FROM tasks WHERE list_id=?1",
            rusqlite::params![list_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(title, "My Task");
    }

    #[test]
    fn test_set_task_tags() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, position) VALUES (?1, 'T', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let task_id = conn.last_insert_rowid();
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES ('work')", []).unwrap();
        let tag_id: i64 = conn.query_row("SELECT id FROM tags WHERE name='work'", [], |r| r.get(0)).unwrap();
        conn.execute(
            "INSERT INTO task_tags (task_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![task_id, tag_id],
        ).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM task_tags WHERE task_id=?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }
}
```

- [ ] **Step 2: Run tests**

```powershell
cd src-tauri && cargo test commands::tasks -- --nocapture
```
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```powershell
git add src-tauri/src/commands/tasks.rs
git commit -m "feat: implement tasks CRUD + dashboard + tags Rust commands"
```

---

## Task 6: Timer Rust commands

**Files:**
- Modify: `src-tauri/src/commands/timer.rs`

- [ ] **Step 1: Replace `src-tauri/src/commands/timer.rs` with full implementation**

```rust
use crate::db::DbState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

pub struct TimerEntry {
    pub start_instant: Instant,
    pub started_at: String,
}

pub struct TimerState(pub Mutex<HashMap<i64, TimerEntry>>);

#[derive(Debug, Serialize, Deserialize)]
pub struct TimerSession {
    pub id: i64,
    pub task_id: i64,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub duration_seconds: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActiveTimer {
    pub task_id: i64,
    pub elapsed_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartTimerResult {
    pub stopped_task_id: Option<i64>,
}

fn stop_timer_inner(task_id: i64, entry: TimerEntry, conn: &rusqlite::Connection) -> Result<(), String> {
    let elapsed = entry.start_instant.elapsed().as_secs() as i64;
    let stopped_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO timer_sessions (task_id, started_at, stopped_at, duration_seconds)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![task_id, entry.started_at, stopped_at, elapsed],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn start_timer(
    task_id: i64,
    timer_state: State<TimerState>,
    db_state: State<DbState>,
) -> Result<StartTimerResult, String> {
    let mut timers = timer_state.0.lock().map_err(|e| e.to_string())?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;

    let stopped_task_id = if !timers.is_empty() {
        let stopped_id = *timers.keys().next().unwrap();
        let entry = timers.remove(&stopped_id).unwrap();
        stop_timer_inner(stopped_id, entry, &conn)?;
        Some(stopped_id)
    } else {
        None
    };

    let started_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    timers.insert(task_id, TimerEntry { start_instant: Instant::now(), started_at });

    Ok(StartTimerResult { stopped_task_id })
}

#[tauri::command]
pub fn stop_timer(
    task_id: i64,
    timer_state: State<TimerState>,
    db_state: State<DbState>,
) -> Result<(), String> {
    let mut timers = timer_state.0.lock().map_err(|e| e.to_string())?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    if let Some(entry) = timers.remove(&task_id) {
        stop_timer_inner(task_id, entry, &conn)?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_active_timers(timer_state: State<TimerState>) -> Result<Vec<ActiveTimer>, String> {
    let timers = timer_state.0.lock().map_err(|e| e.to_string())?;
    let result = timers
        .iter()
        .map(|(task_id, entry)| ActiveTimer {
            task_id: *task_id,
            elapsed_seconds: entry.start_instant.elapsed().as_secs(),
        })
        .collect();
    Ok(result)
}

#[tauri::command]
pub fn get_timer_sessions(task_id: i64, state: State<DbState>) -> Result<Vec<TimerSession>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, task_id, started_at, stopped_at, duration_seconds
             FROM timer_sessions WHERE task_id=?1 ORDER BY started_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let sessions = stmt
        .query_map(rusqlite::params![task_id], |r| {
            Ok(TimerSession {
                id: r.get(0)?,
                task_id: r.get(1)?,
                started_at: r.get(2)?,
                stopped_at: r.get(3)?,
                duration_seconds: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use crate::db::open_in_memory;
    use std::time::Instant;
    use super::TimerEntry;

    #[test]
    fn test_stop_timer_persists_session() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('L', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, title, position) VALUES (?1, 'T', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let task_id = conn.last_insert_rowid();
        let entry = TimerEntry {
            start_instant: Instant::now(),
            started_at: "2026-06-20T10:00:00".to_string(),
        };
        super::stop_timer_inner(task_id, entry, &conn).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM timer_sessions WHERE task_id=?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_active_timers_tracks_elapsed() {
        use std::collections::HashMap;
        use std::sync::Mutex;
        let mut map: HashMap<i64, TimerEntry> = HashMap::new();
        map.insert(42, TimerEntry {
            start_instant: Instant::now(),
            started_at: "2026-06-20T10:00:00".to_string(),
        });
        let state = Mutex::new(map);
        let timers = state.lock().unwrap();
        assert!(timers.contains_key(&42));
        assert!(timers[&42].start_instant.elapsed().as_secs() < 5);
    }
}
```

- [ ] **Step 2: Run tests**

```powershell
cd src-tauri && cargo test commands::timer -- --nocapture
```
Expected: 2 tests pass.

- [ ] **Step 3: Commit**

```powershell
git add src-tauri/src/commands/timer.rs
git commit -m "feat: implement timer Rust commands with session persistence"
```

---

## Task 7: Settings Rust commands

**Files:**
- Modify: `src-tauri/src/commands/settings.rs`

- [ ] **Step 1: Replace `src-tauri/src/commands/settings.rs`**

```rust
use crate::db::DbState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    pub data_dir: Option<String>,
}

#[tauri::command]
pub fn get_settings(state: State<DbState>) -> Result<Settings, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings")
        .map_err(|e| e.to_string())?;
    let map: HashMap<String, String> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(Settings {
        theme: map.get("theme").cloned().unwrap_or_else(|| "system".to_string()),
        data_dir: map.get("data_dir").cloned(),
    })
}

#[tauri::command]
pub fn set_setting(state: State<DbState>, key: String, value: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn change_data_dir(state: State<DbState>, new_path: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let current_path: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key='data_dir'", [], |r| r.get(0))
        .ok();
    if let Some(src) = current_path {
        let src_db = std::path::Path::new(&src).join("data.db");
        let dst_db = std::path::Path::new(&new_path).join("data.db");
        if src_db.exists() {
            std::fs::create_dir_all(&new_path).map_err(|e| e.to_string())?;
            std::fs::copy(&src_db, &dst_db).map_err(|e| e.to_string())?;
        }
    }
    conn.execute(
        "INSERT INTO settings(key,value) VALUES('data_dir',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![new_path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: Run `cargo check`**

```powershell
cd src-tauri && cargo check
```
Expected: no errors.

- [ ] **Step 3: Commit**

```powershell
git add src-tauri/src/commands/settings.rs
git commit -m "feat: implement settings Rust commands including data dir migration"
```

---

## Task 8: TypeScript types + tauri.ts wrappers

**Files:**
- Create: `src/types.ts`
- Create: `src/lib/tauri.ts`
- Create: `src/lib/timeUtils.ts`
- Create: `src/lib/dateUtils.ts`
- Create: `src/lib/__tests__/timeUtils.test.ts`

- [ ] **Step 1: Create `src/types.ts`**

```ts
export interface List {
  id: number
  title: string
  color: string | null
  position: number
  created_at: string
}

export interface Tag {
  id: number
  name: string
  color: string | null
}

export interface Task {
  id: number
  list_id: number
  title: string
  description: string | null
  priority: 'normal' | 'high'
  due_date: string | null
  completed: boolean
  completed_at: string | null
  position: number
  created_at: string
  updated_at: string
  tags: Tag[]
  total_seconds: number
}

export interface DashboardData {
  overdue: Task[]
  high_priority: Task[]
  upcoming: Task[]
}

export interface TimerSession {
  id: number
  task_id: number
  started_at: string
  stopped_at: string | null
  duration_seconds: number | null
}

export interface ActiveTimer {
  task_id: number
  elapsed_seconds: number
}

export interface Settings {
  theme: 'light' | 'dark' | 'system'
  data_dir: string | null
}

export interface StartTimerResult {
  stopped_task_id: number | null
}
```

- [ ] **Step 2: Create `src/lib/tauri.ts`**

```ts
import { invoke } from '@tauri-apps/api/core'
import type {
  List, Task, Tag, DashboardData, TimerSession, ActiveTimer,
  Settings, StartTimerResult,
} from '../types'

export const api = {
  getLists: () => invoke<List[]>('get_lists'),
  createList: (title: string, color?: string) => invoke<List>('create_list', { title, color }),
  updateList: (id: number, title?: string, color?: string, position?: number) =>
    invoke<List>('update_list', { id, title, color, position }),
  deleteList: (id: number) => invoke<void>('delete_list', { id }),

  getTasks: (listId: number) => invoke<Task[]>('get_tasks', { listId }),
  createTask: (listId: number, title: string, priority?: string, dueDate?: string, description?: string) =>
    invoke<Task>('create_task', { listId, title, priority, dueDate, description }),
  updateTask: (
    id: number,
    fields: { title?: string; description?: string; priority?: string; dueDate?: string; completed?: boolean; position?: number }
  ) => invoke<Task>('update_task', { id, ...fields }),
  deleteTask: (id: number) => invoke<void>('delete_task', { id }),
  getDashboardTasks: () => invoke<DashboardData>('get_dashboard_tasks'),
  setTaskTags: (taskId: number, tagNames: string[]) => invoke<Tag[]>('set_task_tags', { taskId, tagNames }),
  getAllTags: () => invoke<Tag[]>('get_all_tags'),

  startTimer: (taskId: number) => invoke<StartTimerResult>('start_timer', { taskId }),
  stopTimer: (taskId: number) => invoke<void>('stop_timer', { taskId }),
  getActiveTimers: () => invoke<ActiveTimer[]>('get_active_timers'),
  getTimerSessions: (taskId: number) => invoke<TimerSession[]>('get_timer_sessions', { taskId }),

  getSettings: () => invoke<Settings>('get_settings'),
  setSetting: (key: string, value: string) => invoke<void>('set_setting', { key, value }),
  changeDataDir: (newPath: string) => invoke<void>('change_data_dir', { newPath }),
}
```

- [ ] **Step 3: Create `src/lib/timeUtils.ts`**

```ts
export function formatTotal(totalSeconds: number): string {
  if (totalSeconds === 0) return ''
  const h = Math.floor(totalSeconds / 3600)
  const m = Math.floor((totalSeconds % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

export function formatLive(seconds: number): string {
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
}
```

- [ ] **Step 4: Create `src/lib/dateUtils.ts`**

```ts
export function today(): string {
  return new Date().toISOString().split('T')[0]
}

export function isOverdue(dueDate: string | null): boolean {
  if (!dueDate) return false
  return dueDate < today()
}

export function isDueToday(dueDate: string | null): boolean {
  if (!dueDate) return false
  return dueDate === today()
}

export function formatDueDate(dueDate: string): string {
  const d = new Date(dueDate + 'T00:00:00')
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}
```

- [ ] **Step 5: Write tests**

Create `src/lib/__tests__/timeUtils.test.ts`:
```ts
import { describe, it, expect } from 'vitest'
import { formatTotal, formatLive } from '../timeUtils'

describe('formatTotal', () => {
  it('returns empty string for 0 seconds', () => {
    expect(formatTotal(0)).toBe('')
  })
  it('formats minutes only', () => {
    expect(formatTotal(90)).toBe('1m')
  })
  it('formats hours and minutes', () => {
    expect(formatTotal(3661)).toBe('1h 1m')
  })
})

describe('formatLive', () => {
  it('pads minutes and seconds', () => {
    expect(formatLive(65)).toBe('01:05')
  })
  it('formats zero as 00:00', () => {
    expect(formatLive(0)).toBe('00:00')
  })
})
```

- [ ] **Step 6: Run tests**

```powershell
npm test
```
Expected: 5 tests pass.

- [ ] **Step 7: Commit**

```powershell
git add src/types.ts src/lib/ 
git commit -m "feat: add TypeScript types, tauri API wrappers, and utility functions"
```

---

## Task 9: Zustand stores

**Files:**
- Create: `src/store/listsStore.ts`
- Create: `src/store/tasksStore.ts`
- Create: `src/store/timerStore.ts`
- Create: `src/store/settingsStore.ts`

- [ ] **Step 1: Create `src/store/listsStore.ts`**

```ts
import { create } from 'zustand'
import { api } from '../lib/tauri'
import type { List } from '../types'

interface ListsStore {
  lists: List[]
  loading: boolean
  load: () => Promise<void>
  create: (title: string, color?: string) => Promise<List>
  update: (id: number, title?: string, color?: string) => Promise<void>
  remove: (id: number) => Promise<void>
}

export const useListsStore = create<ListsStore>((set) => ({
  lists: [],
  loading: false,
  load: async () => {
    set({ loading: true })
    const lists = await api.getLists()
    set({ lists, loading: false })
  },
  create: async (title, color) => {
    const list = await api.createList(title, color)
    set((s) => ({ lists: [...s.lists, list] }))
    return list
  },
  update: async (id, title, color) => {
    const updated = await api.updateList(id, title, color)
    set((s) => ({ lists: s.lists.map((l) => (l.id === id ? updated : l)) }))
  },
  remove: async (id) => {
    await api.deleteList(id)
    set((s) => ({ lists: s.lists.filter((l) => l.id !== id) }))
  },
}))
```

- [ ] **Step 2: Create `src/store/tasksStore.ts`**

```ts
import { create } from 'zustand'
import { api } from '../lib/tauri'
import type { Task, DashboardData } from '../types'

interface TasksStore {
  tasks: Record<number, Task[]>
  dashboard: DashboardData | null
  loadList: (listId: number) => Promise<void>
  loadDashboard: () => Promise<void>
  create: (listId: number, title: string) => Promise<Task>
  update: (id: number, listId: number, fields: Parameters<typeof api.updateTask>[1]) => Promise<void>
  remove: (id: number, listId: number) => Promise<void>
  setTags: (taskId: number, listId: number, tagNames: string[]) => Promise<void>
}

export const useTasksStore = create<TasksStore>((set) => ({
  tasks: {},
  dashboard: null,
  loadList: async (listId) => {
    const tasks = await api.getTasks(listId)
    set((s) => ({ tasks: { ...s.tasks, [listId]: tasks } }))
  },
  loadDashboard: async () => {
    const dashboard = await api.getDashboardTasks()
    set({ dashboard })
  },
  create: async (listId, title) => {
    const task = await api.createTask(listId, title)
    set((s) => ({ tasks: { ...s.tasks, [listId]: [...(s.tasks[listId] ?? []), task] } }))
    return task
  },
  update: async (id, listId, fields) => {
    const updated = await api.updateTask(id, fields)
    set((s) => ({
      tasks: {
        ...s.tasks,
        [listId]: (s.tasks[listId] ?? []).map((t) => (t.id === id ? updated : t)),
      },
    }))
  },
  remove: async (id, listId) => {
    await api.deleteTask(id)
    set((s) => ({
      tasks: { ...s.tasks, [listId]: (s.tasks[listId] ?? []).filter((t) => t.id !== id) },
    }))
  },
  setTags: async (taskId, listId, tagNames) => {
    const tags = await api.setTaskTags(taskId, tagNames)
    set((s) => ({
      tasks: {
        ...s.tasks,
        [listId]: (s.tasks[listId] ?? []).map((t) =>
          t.id === taskId ? { ...t, tags } : t
        ),
      },
    }))
  },
}))
```

- [ ] **Step 3: Create `src/store/timerStore.ts`**

```ts
import { create } from 'zustand'
import { api } from '../lib/tauri'

interface TimerStore {
  activeTaskId: number | null
  elapsedSeconds: number
  _intervalId: ReturnType<typeof setInterval> | null
  start: (taskId: number) => Promise<number | null>
  stop: (taskId: number) => Promise<void>
  _poll: () => Promise<void>
}

export const useTimerStore = create<TimerStore>((set, get) => ({
  activeTaskId: null,
  elapsedSeconds: 0,
  _intervalId: null,
  start: async (taskId) => {
    const result = await api.startTimer(taskId)
    const prev = get()._intervalId
    if (prev) clearInterval(prev)
    const id = setInterval(() => get()._poll(), 1000)
    set({ activeTaskId: taskId, elapsedSeconds: 0, _intervalId: id })
    return result.stopped_task_id
  },
  stop: async (taskId) => {
    await api.stopTimer(taskId)
    const id = get()._intervalId
    if (id) clearInterval(id)
    set({ activeTaskId: null, elapsedSeconds: 0, _intervalId: null })
  },
  _poll: async () => {
    const timers = await api.getActiveTimers()
    if (timers.length === 0) {
      const id = get()._intervalId
      if (id) clearInterval(id)
      set({ activeTaskId: null, elapsedSeconds: 0, _intervalId: null })
      return
    }
    set({ activeTaskId: timers[0].task_id, elapsedSeconds: timers[0].elapsed_seconds })
  },
}))
```

- [ ] **Step 4: Create `src/store/settingsStore.ts`**

```ts
import { create } from 'zustand'
import { api } from '../lib/tauri'
import type { Settings } from '../types'

interface SettingsStore {
  settings: Settings | null
  load: () => Promise<void>
  setTheme: (theme: 'light' | 'dark' | 'system') => Promise<void>
  changeDataDir: (path: string) => Promise<void>
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: null,
  load: async () => {
    const settings = await api.getSettings()
    set({ settings })
    applyTheme(settings.theme)
  },
  setTheme: async (theme) => {
    await api.setSetting('theme', theme)
    set((s) => ({ settings: s.settings ? { ...s.settings, theme } : null }))
    applyTheme(theme)
  },
  changeDataDir: async (path) => {
    await api.changeDataDir(path)
    set((s) => ({ settings: s.settings ? { ...s.settings, data_dir: path } : null }))
  },
}))

function applyTheme(theme: 'light' | 'dark' | 'system') {
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  const isDark = theme === 'dark' || (theme === 'system' && prefersDark)
  document.documentElement.classList.toggle('dark', isDark)
}
```

- [ ] **Step 5: Commit**

```powershell
git add src/store/
git commit -m "feat: add Zustand stores for lists, tasks, timer, settings"
```

---

## Task 10: App layout, Sidebar, and routing

**Files:**
- Modify: `src/App.tsx`
- Create: `src/components/Sidebar.tsx`

(Full code in plan — see spec section 4 for UI layout details)

---

## Task 11: TaskCard + ListDetail page

**Files:**
- Create: `src/components/TaskCard.tsx`
- Create: `src/components/TimerWidget.tsx`
- Create: `src/pages/ListDetail.tsx`

---

## Task 12: TaskEditor panel + Markdown

**Files:**
- Create: `src/components/TaskEditor.tsx`
- Create: `src/components/MarkdownRenderer.tsx`
- Create: `src/components/TagInput.tsx`
- Create: `src/components/__tests__/MarkdownRenderer.test.tsx`

---

## Task 13: Dashboard page

**Files:**
- Create: `src/pages/Dashboard.tsx`
- Create: `src/components/__tests__/TaskCard.test.tsx`

---

## Task 14: Settings page

**Files:**
- Create: `src/pages/Settings.tsx`

---

## Task 15: Full test suite + production build

- Run `npm test` (all Vitest tests)
- Run `cd src-tauri && cargo test`
- Run `npx tauri build`
- Final commit
