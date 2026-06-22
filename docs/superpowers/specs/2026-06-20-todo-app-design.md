# Smart Todo — Design Specification

**Date:** 2026-06-20  
**Status:** Approved  

---

## 1. Overview

Smart Todo is a cross-platform desktop application (Windows, Linux, macOS) for managing multiple todo lists with rich task metadata, Markdown support, a built-in time tracker, and a high-priority dashboard. Data is stored locally in SQLite and sync is achieved by pointing the data directory at an iCloud Drive or OneDrive folder.

**Tech stack:**
- **Frontend:** React 18 + TypeScript, Vite, Zustand, react-markdown, Tailwind CSS
- **Backend:** Tauri 2 (Rust), rusqlite (SQLite)
- **Testing:** Vitest + Testing Library (frontend), Rust `#[cfg(test)]` (backend), Playwright + Tauri WebDriver (E2E)

---

## 2. Architecture

### Directory Structure

```
smart-todo/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Tauri app entry, command registration
│   │   ├── db.rs                # SQLite connection, migrations
│   │   └── commands/
│   │       ├── lists.rs         # CRUD for todo lists
│   │       ├── tasks.rs         # CRUD for tasks + tags
│   │       ├── timer.rs         # Timer start/stop/query
│   │       └── settings.rs      # Data directory + theme config
│   └── Cargo.toml
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── store/
│   │   ├── listsStore.ts
│   │   ├── tasksStore.ts
│   │   └── timerStore.ts
│   ├── pages/
│   │   ├── Dashboard.tsx
│   │   └── ListDetail.tsx
│   ├── components/
│   │   ├── Sidebar.tsx
│   │   ├── TaskCard.tsx
│   │   ├── TaskEditor.tsx
│   │   ├── TimerWidget.tsx
│   │   └── MarkdownRenderer.tsx
│   └── lib/
│       └── tauri.ts             # Typed invoke() wrappers
├── package.json
└── vite.config.ts
```

### Data Flow

React components call typed `invoke()` wrappers in `lib/tauri.ts` → Tauri Rust commands → SQLite file on disk. Timer state lives in a Rust `Mutex<HashMap<i64, Instant>>` so elapsed time is tracked natively and survives React re-renders. The React `timerStore` polls `get_active_timers()` every second when any timer is active to update the UI counter.

---

## 3. Data Model

```sql
CREATE TABLE lists (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  title      TEXT NOT NULL,
  color      TEXT,
  position   INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE tasks (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  list_id      INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
  title        TEXT NOT NULL,
  description  TEXT,
  priority     TEXT CHECK(priority IN ('normal', 'high')) DEFAULT 'normal',
  due_date     TEXT,
  completed    BOOLEAN DEFAULT 0,
  completed_at TEXT,
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE tags (
  id    INTEGER PRIMARY KEY AUTOINCREMENT,
  name  TEXT NOT NULL UNIQUE,
  color TEXT
);

CREATE TABLE task_tags (
  task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (task_id, tag_id)
);

CREATE TABLE timer_sessions (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id          INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  started_at       TEXT NOT NULL,
  stopped_at       TEXT,
  duration_seconds INTEGER
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
-- Known keys: 'data_dir', 'theme' ('light' | 'dark' | 'system')
```

Total tracked time per task = `SELECT SUM(duration_seconds) FROM timer_sessions WHERE task_id = ?` plus current active session elapsed (if running).

---

## 4. UI Structure

### Layout

Two-panel layout: fixed left sidebar (240px) + main content area.

```
┌─────────────────────────────────────────────────────────┐
│  Smart Todo                                [⚙ Settings] │
├──────────────────┬──────────────────────────────────────┤
│  Dashboard       │                                      │
│  ──────────────  │   Main content area                  │
│  📋 Work         │   (Dashboard or List Detail)         │
│  📋 Personal     │                                      │
│  📋 Shopping     │                                      │
│  ──────────────  │                                      │
│  [+ New List]    │                                      │
│                  │                                      │
│  🏷 Tags         │                                      │
│  work  design    │                                      │
└──────────────────┴──────────────────────────────────────┘
```

### Dashboard Screen

Shown on launch. Three sections, all scoped to incomplete tasks:

1. **Due Today / Overdue** — tasks where `due_date <= today`, sorted: overdue first, then by priority
2. **High Priority** — all `priority = 'high'` tasks across all lists (excluding ones already in section 1)
3. **Upcoming** — tasks where `due_date` is within the next 7 days (excluding sections 1 & 2)

Each section shows an empty-state message when no tasks qualify.

### List Detail Screen

Shown when a list is selected in the sidebar.

- Task list ordered by `position` (default), with sort options: position, due date, priority
- "Add task" row at the bottom — click to create inline
- Each **TaskCard** shows: title, tag chips, due date badge (red if overdue), high-priority indicator (⚑), timer button + total tracked time
- Completed tasks collapsed into a "Completed (N)" disclosure at the bottom

### Task Editor Panel

Opens as a right-side drawer when a task card is clicked. Does not replace the list view.

- **Title** — editable inline at the top
- **Priority** — toggle button: Normal / High
- **Due date** — date picker (clearable)
- **Tags** — autocomplete input; creates new tags on Enter; existing tags shown as removable chips
- **Description** — Markdown editor with toggle between edit and preview modes
- **Timer widget** — Start/Stop button, live elapsed counter, list of past sessions (date + duration)
- **Delete task** — at the bottom, with confirmation

### Settings Screen

Accessible via gear icon (top-right). Full-panel view.

- **Data directory** — shows current path, "Change…" button opens native folder picker
- **Theme** — segmented control: Light / Dark / System

---

## 5. Timer Feature

### Rust (`timer.rs`)

- Global state: `Mutex<HashMap<i64, TimerEntry>>` where `TimerEntry` holds `start_instant: Instant` and `started_at: String`
- `start_timer(task_id: i64)` — if another timer is active, stop it first (persist session to DB), then insert new entry
- `stop_timer(task_id: i64)` — remove from map, compute `duration_seconds`, insert `timer_sessions` row
- `get_active_timers() -> Vec<ActiveTimer>` — returns `[{task_id, elapsed_seconds}]` for all active timers

### React (`timerStore.ts`)

- Polls `get_active_timers()` every 1 second when any timer is active
- Exposes `{ activeTaskId, elapsedSeconds, startTimer, stopTimer }`
- `TimerWidget` receives `task_id`, reads store to determine if this task is active

### UX Rules

- Only one timer active at a time — starting a new one auto-stops the previous with a toast notification
- Timer continues even if the Task Editor panel is closed
- Displayed as `Xh Ym` for totals, `MM:SS` for live counter

---

## 6. Data Sync

No custom sync backend. The app stores its SQLite file at the path set in `settings.data_dir`. Default: Tauri's `$APPDATA/smart-todo/data.db`.

Users who want iCloud or OneDrive sync change the data directory to a folder inside their iCloud Drive or OneDrive folder. The OS sync client handles the rest. The Settings screen documents this with a brief note.

**Migration:** When the user changes data directory, the app copies the existing `.db` file to the new location before switching, so no data is lost.

---

## 7. Tauri Commands (Public API)

All commands are registered in `main.rs` and callable from React via `invoke()`.

| Command | Args | Returns |
|---|---|---|
| `get_lists` | — | `List[]` |
| `create_list` | `title, color` | `List` |
| `update_list` | `id, title?, color?, position?` | `List` |
| `delete_list` | `id` | — |
| `get_tasks` | `list_id` | `Task[]` (with tags) |
| `create_task` | `list_id, title, priority?, due_date?, description?` | `Task` |
| `update_task` | `id, title?, priority?, due_date?, description?, completed?, position?` | `Task` |
| `delete_task` | `id` | — |
| `get_dashboard_tasks` | — | `DashboardData` |
| `set_task_tags` | `task_id, tag_names[]` | `Tag[]` |
| `get_all_tags` | — | `Tag[]` |
| `start_timer` | `task_id` | `{ stopped_task_id?: i64 }` |
| `stop_timer` | `task_id` | — |
| `get_active_timers` | — | `ActiveTimer[]` |
| `get_timer_sessions` | `task_id` | `TimerSession[]` |
| `get_settings` | — | `Settings` |
| `set_setting` | `key, value` | — |
| `change_data_dir` | `new_path` | — |

---

## 8. Testing Strategy

### Rust (unit + integration)
- `db.rs` — migrations run cleanly, schema matches spec
- `tasks.rs` — CRUD against `":memory:"` SQLite
- `timer.rs` — start/stop/auto-stop logic, elapsed calculation, session persistence

### React (Vitest + Testing Library)
- `TaskCard` — renders title, tags, due date, priority badge, timer button
- `Dashboard` — correct sections, empty states, overdue highlighting
- `timerStore` — state transitions, elapsed formatting
- `MarkdownRenderer` — renders Markdown, rejects `<script>` tags (XSS)

### E2E (Playwright + Tauri WebDriver)
- Create list → add task → set priority/due date/tags → verify appears on dashboard
- Start timer → stop timer → verify total time displayed
- Change data directory → verify data persists at new path

---

## 9. Implementation Phases

### Phase 1 — Foundation
- Tauri project scaffold + Vite + React + TypeScript + Tailwind
- SQLite connection + migrations (`db.rs`)
- Basic Tauri commands: lists + tasks CRUD
- Sidebar + List Detail screen (no task editor yet)

### Phase 2 — Task Editor & Markdown
- Task Editor panel (title, priority, due date, tags)
- Markdown editor + preview (`react-markdown` + `remark-gfm`)
- Tag system (autocomplete, create, assign)

### Phase 3 — Timer
- Rust timer state + commands
- `timerStore` polling
- `TimerWidget` component + session history in Task Editor

### Phase 4 — Dashboard
- `get_dashboard_tasks` command (overdue, high-priority, upcoming queries)
- Dashboard screen with three sections

### Phase 5 — Settings & Data Directory
- Settings screen
- `change_data_dir` command with file copy
- Theme toggle (light/dark/system via Tailwind `dark:` classes)

### Phase 6 — Polish & Tests
- Full unit test suite (Vitest + Rust tests)
- E2E tests (Playwright)
- App icons, window title, about page
- Production build + installer (Tauri bundler: `.msi`, `.dmg`, `.AppImage`)
