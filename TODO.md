# Smart Todo — Implementation TODO

## Phase 1 — Foundation

- [x] Task 1: Initialize Tauri 2 + React TypeScript project
- [ ] Task 2: Configure Tailwind CSS + Vite
- [ ] Task 3: Database module — connection + migrations (`db.rs`)
- [ ] Task 4: Lists CRUD Rust commands (`lists.rs`)
- [ ] Task 5: Tasks CRUD Rust commands (`tasks.rs`)

## Phase 2 — Backend & TS Layer

- [ ] Task 6: Timer Rust commands (`timer.rs`)
- [ ] Task 7: Settings Rust commands (`settings.rs`)
- [ ] Task 8: TypeScript types + `tauri.ts` wrappers + utility functions
- [ ] Task 9: Zustand stores (lists, tasks, timer, settings)

## Phase 3 — UI Core

- [ ] Task 10: App layout, Sidebar, and routing
- [ ] Task 11: TaskCard + ListDetail page

## Phase 4 — Rich Features

- [ ] Task 12: TaskEditor panel + Markdown + TagInput

## Phase 5 — Dashboard & Settings UI

- [ ] Task 13: Dashboard page (overdue / high priority / upcoming)
- [ ] Task 14: Settings page (theme + data directory)

## Phase 6 — Tests & Build

- [ ] Task 15: Full test suite + production build

---

## Known Issues / Backlog

- [ ] Fix Tauri plugin capability entries (dialog, fs need entries in `capabilities/default.json`)
- [ ] Remove `tauri-plugin-shell` (not needed, security risk)
- [ ] Fix `tauri-plugin-opener` Rust/JS mismatch (either add to Cargo.toml or remove from package.json)
- [ ] Add Vitest configuration to `vite.config.ts`
- [ ] Add `test` script to `package.json`
- [ ] Set a restrictive CSP in `tauri.conf.json` before shipping
