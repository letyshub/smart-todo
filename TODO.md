# Smart Todo — Implementation TODO

## Phase 1 — Foundation

- [x] Task 1: Initialize Tauri 2 + React TypeScript project
- [x] Task 2: Configure Tailwind CSS + Vite
- [x] Task 3: Database module — connection + migrations (`db.rs`)
- [x] Task 4: Lists CRUD Rust commands (`lists.rs`)
- [x] Task 5: Tasks CRUD Rust commands (`tasks.rs`)

## Phase 2 — Backend & TS Layer

- [x] Task 6: Timer Rust commands (`timer.rs`)
- [x] Task 7: Settings Rust commands (`settings.rs`)
- [x] Task 8: TypeScript types + `tauri.ts` wrappers + utility functions
- [x] Task 9: Zustand stores (lists, tasks, timer, settings)

## Phase 3 — UI Core

- [x] Task 10: App layout, Sidebar, and routing
- [x] Task 11: TaskCard + ListDetail page

## Phase 4 — Rich Features

- [x] Task 12: TaskEditor panel + Markdown + TagInput

## Phase 5 — Dashboard & Settings UI

- [x] Task 13: Dashboard page (overdue / high priority / upcoming)
- [x] Task 14: Settings page (theme + data directory)

## Phase 6 — Tests & Build

- [x] Task 15: Full test suite + production build

---

## Known Issues / Backlog

- [x] Fix Tauri plugin capability entries (dialog, fs need entries in `capabilities/default.json`)
- [x] Remove `tauri-plugin-shell` (not needed, security risk)
- [x] Fix `tauri-plugin-opener` Rust/JS mismatch (either add to Cargo.toml or remove from package.json)
- [x] Add Vitest configuration to `vite.config.ts`
- [x] Add `test` script to `package.json`
- [x] Set a restrictive CSP in `tauri.conf.json` before shipping
