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
│       ├── sync/         # Cross-device sync over a shared cloud folder
│       │   ├── mod.rs        # Engine: push/pull passes, retries, compaction
│       │   ├── model.rs      # Which tables and columns sync (data-driven)
│       │   ├── schema.rs     # Sync tables, uuid columns, capture triggers
│       │   ├── op.rs         # Op + Rev (Lamport clock + device id)
│       │   ├── meta.rs       # Device identity, Lamport clock
│       │   ├── row.rs        # Row <-> field map, uuid <-> local id
│       │   ├── capture.rs    # Outbox -> ops (per-field diff against shadow)
│       │   ├── apply.rs      # Remote ops -> database, conflict detection
│       │   └── store.rs      # Sync folder layout, segment read/write
│       └── commands/
│           ├── mod.rs
│           ├── lists.rs      # Lists CRUD
│           ├── tasks.rs      # Tasks CRUD + tags + dashboard
│           ├── timer.rs      # Timer start/stop/query + TimerState
│           ├── settings.rs   # Theme, widths, database location
│           └── sync.rs       # Sync folder, sync now, conflicts
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
    │   ├── settingsStore.ts
    │   └── syncStore.ts
    ├── pages/
    │   ├── Dashboard.tsx  # Overdue / High Priority / Upcoming
    │   ├── ListDetail.tsx # Tasks for one list
    │   └── Settings.tsx   # Theme + data directory
    └── components/
        ├── Sidebar.tsx
        ├── TaskCard.tsx
        ├── TaskEditor.tsx    # Right-side drawer
        ├── TimerWidget.tsx   # Start/stop + live counter
        ├── SyncConflicts.tsx # Banner for edits overridden by another machine
        ├── MarkdownRenderer.tsx
        └── TagInput.tsx
```

## Key Design Decisions

- **All data access through Tauri commands** — frontend never touches SQLite directly
- **Timer state in Rust** — `Mutex<HashMap<i64, TimerEntry>>` for accurate wall-clock tracking across React re-renders
- **The database is always local** — never in a cloud-synced folder. SQLite in WAL mode is three files that must stay mutually consistent, and a sync client copies them independently; that is what corrupts the database
- **Sync ships a change log, not the database** — each device writes only `devices/<its own id>/ops-*.jsonl` inside the shared folder, so no file ever has two writers and the cloud provider has nothing to reconcile
- **Merging is per field** — each field carries a Lamport revision and the revision it was written on top of, so edits to different fields merge; same-field divergence resolves by highest revision and is reported on the machine whose value lost
- **Changes are captured by triggers** — a new command cannot forget to record what it changed
- **One active timer at a time** — `start_timer` auto-stops any running timer before starting a new one

## Data Locations

- **Database:** `$LOCALAPPDATA/smart-todo/data.db` (Windows) or the platform equivalent. Not configurable.
- **Sync folder:** chosen in Settings → "Choose sync folder…", typically inside OneDrive or iCloud Drive. Holds `devices/<device-id>/ops-*.jsonl` plus `meta.json` per device.
- **Pointers:** `sync-dir.txt` in the config directory. A `data-dir.txt` left by an older version triggers a one-time migration: the database is copied back to local storage and that folder becomes the sync folder.

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

Six user tables: `lists`, `tasks`, `tags`, `task_tags`, `timer_sessions`, `settings`.
Syncable rows carry a `uuid` column; row ids stay local.

Sync bookkeeping: `sync_meta`, `sync_outbox`, `sync_field_revs`, `sync_shadow`,
`sync_files`, `sync_deferred`, `sync_alias`, `sync_tombstones`, `sync_conflicts`.

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
| 7 — Cross-device sync | Local-first DB + op-log sync, conflict UI ([#3](https://github.com/letyshub/smart-todo/issues/3)) | ✅ Done |
