# Productivity Panel — Design Spec

**Date:** 2026-08-14
**Status:** Approved

## Overview

Add a permanent right-side productivity panel to the Dashboard showing weekly statistics: completed tasks, tracked time, and on-time completion rate. Data comes from a new dedicated Rust Tauri command that queries SQLite directly.

## Goals

- Give the user a quick weekly summary of their productivity without leaving the Dashboard
- Reuse existing data already stored in `tasks` and `timer_sessions` tables
- Keep the implementation clean: separate Rust command, separate React component

## Out of Scope

- Historical charts or graphs
- Per-list or per-tag breakdowns
- Configurable time window (always last 7 days)
- Exporting stats

---

## Architecture

### 1. Backend — new Rust command

**File:** `src-tauri/src/commands/stats.rs`

New struct returned by the command:

```rust
#[derive(Debug, Serialize)]
pub struct ProductivityStats {
    pub tasks_completed_week: i64,
    pub total_seconds_week: i64,
    pub on_time_count: i64,
    pub late_count: i64,
}
```

**Command:** `get_productivity_stats() -> Result<ProductivityStats, String>`

Three SQL queries executed against the existing schema:

1. **Completed tasks this week:**
   ```sql
   SELECT COUNT(*) FROM tasks
   WHERE completed = 1
     AND completed_at >= datetime('now', '-7 days')
     AND parent_task_id IS NULL
   ```

2. **Total tracked seconds this week:**
   ```sql
   SELECT COALESCE(SUM(duration_seconds), 0) FROM timer_sessions
   WHERE started_at >= datetime('now', '-7 days')
   ```

3. **On-time vs late (tasks with a due_date, completed this week):**
   ```sql
   SELECT
     SUM(CASE WHEN date(completed_at) <= due_date THEN 1 ELSE 0 END),
     SUM(CASE WHEN date(completed_at) > due_date  THEN 1 ELSE 0 END)
   FROM tasks
   WHERE completed = 1
     AND completed_at >= datetime('now', '-7 days')
     AND due_date IS NOT NULL
     AND parent_task_id IS NULL
   ```
   Tasks without a `due_date` are excluded from both counters.

The command is registered in `src-tauri/src/main.rs` alongside existing commands.

### 2. Frontend — types and invoke wrapper

**`src/types.ts`** — add:
```typescript
export interface ProductivityStats {
  tasks_completed_week: number
  total_seconds_week: number
  on_time_count: number
  late_count: number
}
```

**`src/lib/tauri.ts`** — add:
```typescript
export const getProductivityStats = () =>
  invoke<ProductivityStats>('get_productivity_stats')
```

### 3. Frontend — ProductivityPanel component

**File:** `src/components/ProductivityPanel.tsx`

Props: `stats: ProductivityStats`

Renders three stat tiles stacked vertically:

| Tile | Value | Label |
|---|---|---|
| Ukończone | `tasks_completed_week` | zadań w tym tygodniu |
| Czas pracy | `formatTotal(total_seconds_week) \|\| '0m'` | zarejestrowany czas |
| Terminowość | `on_time_count / (on_time_count + late_count)` or `—` | ukończonych na czas |

- **Terminowość** shows `—` when `on_time_count + late_count === 0` (no tasks with due dates completed this week)
- Time formatted via existing `formatTotal` from `src/lib/timeUtils.ts` (returns `""` for 0 — panel shows `"0m"` as fallback)
- Panel width: `w-56`, left border separating it from task list
- Header: "Ten tydzień" label at the top of the panel

### 4. Dashboard layout change

**`src/pages/Dashboard.tsx`**

Current layout:
```
flex h-full
  └─ flex-1 overflow-y-auto   (task sections)
  └─ TaskEditor (conditional)
```

New layout:
```
flex h-full
  └─ flex-1 overflow-y-auto   (task sections)
  └─ ProductivityPanel w-56   (permanent, right side)
  └─ TaskEditor (conditional, overlays from right)
```

`ProductivityStats` is fetched in `Dashboard.tsx` via `useState<ProductivityStats | null>` + `useEffect` alongside the existing `loadDashboard()` call. Stats are not stored in Zustand (not shared with other views).

---

## Data Flow

```
Dashboard mounts
  → loadDashboard()        → get_dashboard_tasks (Rust) → DashboardData
  → getProductivityStats() → get_productivity_stats (Rust) → ProductivityStats
                                          ↓
                               ProductivityPanel renders stat tiles
```

---

## Error Handling

- If `getProductivityStats()` fails, the panel shows a subtle error state ("Nie udało się załadować statystyk") without blocking the rest of the dashboard.
- Stats refresh only on Dashboard mount (same cadence as task sections).

---

## Testing

**Rust (`cargo test`):**
- `test_productivity_stats_empty` — no tasks/sessions → all zeros
- `test_productivity_stats_completed_week` — tasks completed within 7 days are counted
- `test_productivity_stats_outside_window` — tasks older than 7 days are excluded
- `test_on_time_vs_late` — correct split when due_date before/after completed_at

**Frontend (Vitest):**
- `ProductivityPanel` renders correct values for given props
- Shows `—` for terminowość when no tasks with due dates completed
- Shows `0m` when total_seconds_week is 0 (formatTotal returns "" for 0)
