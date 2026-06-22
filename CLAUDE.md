# Smart Todo — CLAUDE.md

## Project Overview

Cross-platform desktop todo application built with Tauri 2 (Rust) + React 18 (TypeScript).

**Target platforms:** Windows, Linux, macOS

## Tech Stack

| Layer | Technology |
|---|---|
| Frontend | React 18, TypeScript 5, Vite 5, Tailwind CSS 3 |
| State | Zustand 4 |
| Markdown | react-markdown 9 + remark-gfm |
| Backend | Tauri 2 (Rust) |
| Database | SQLite via rusqlite 0.31 (bundled) |
| Date handling | chrono 0.4 |
| Testing (FE) | Vitest, @testing-library/react |
| Testing (Rust) | cargo test with in-memory SQLite |

## Architecture

```
smart-todo/
├── src-tauri/            # Rust backend
│   └── src/
│       ├── main.rs       # Tauri entry, registers commands + state
│       ├── lib.rs        # Plugin init
│       ├── db.rs         # SQLite connection, migrations, DbState
│       └── commands/
│           ├── mod.rs
│           ├── lists.rs      # Lists CRUD
│           ├── tasks.rs      # Tasks CRUD + tags + dashboard
│           ├── timer.rs      # Timer start/stop/query + TimerState
│           └── settings.rs   # Settings + data dir migration
└── src/                  # React frontend
    ├── types.ts           # TypeScript types (mirror Rust structs)
    ├── lib/
    │   ├── tauri.ts       # Typed invoke() wrappers
    │   ├── timeUtils.ts   # Format seconds → "Xh Ym", "MM:SS"
    │   └── dateUtils.ts   # isOverdue, isToday, formatDueDate
    ├── store/
    │   ├── listsStore.ts
    │   ├── tasksStore.ts
    │   ├── timerStore.ts
    │   └── settingsStore.ts
    ├── pages/
    │   ├── Dashboard.tsx  # Overdue / High Priority / Upcoming
    │   ├── ListDetail.tsx # Tasks for one list
    │   └── Settings.tsx   # Theme + data directory
    └── components/
        ├── Sidebar.tsx
        ├── TaskCard.tsx
        ├── TaskEditor.tsx    # Right-side drawer
        ├── TimerWidget.tsx   # Start/stop + live counter
        ├── MarkdownRenderer.tsx
        └── TagInput.tsx
```

## Key Design Decisions

- **All data access through Tauri commands** — frontend never touches SQLite directly
- **Timer state in Rust** — `Mutex<HashMap<i64, TimerEntry>>` for accurate wall-clock tracking across React re-renders
- **Data sync via configurable data directory** — users point SQLite file at iCloud/OneDrive folder; no custom sync backend
- **One active timer at a time** — `start_timer` auto-stops any running timer before starting a new one

## Data Directory

Default: Tauri `$APPDATA/smart-todo/data.db`
Configurable via Settings → "Change data directory…" (copies DB to new location)

## Running the App

```powershell
# Development
npx tauri dev

# Production build
npx tauri build
```

## Running Tests

```powershell
# Frontend (Vitest)
npm test

# Rust
cd src-tauri && cargo test
```

## Database Schema

Six tables: `lists`, `tasks`, `tags`, `task_tags`, `timer_sessions`, `settings`

See `src-tauri/src/db.rs` for full schema with migrations.

## Spec & Plan

- **Design spec:** `docs/superpowers/specs/2026-06-20-todo-app-design.md`
- **Implementation plan:** `docs/superpowers/plans/2026-06-20-smart-todo.md`

## Implementation Phases

| Phase | Tasks | Status |
|---|---|---|
| 1 — Foundation | Scaffold, Tailwind, DB, Lists CRUD | ✅ Done |
| 2 — Task Editor | Tasks CRUD, Settings, TS layer, Stores | ✅ Done |
| 3 — UI Core | App layout, Sidebar, TaskCard, ListDetail | ✅ Done |
| 4 — Rich Features | TaskEditor, Markdown, Tags | ✅ Done |
| 5 — Dashboard & Settings | Dashboard page, Settings page | ✅ Done |
| 6 — Tests & Build | Full test suite, production build | ✅ Done |
