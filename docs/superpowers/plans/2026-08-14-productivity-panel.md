# Productivity Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a permanent right-side productivity panel to the Dashboard showing weekly stats: completed tasks, tracked time, and on-time completion rate.

**Architecture:** A new Rust command `get_productivity_stats` queries SQLite with three SQL aggregations and returns a `ProductivityStats` struct. A new `ProductivityPanel` React component receives the stats as props and renders three stat tiles. `Dashboard.tsx` fetches the stats alongside the existing dashboard tasks and places the panel in the right column.

**Tech Stack:** Rust (rusqlite, Tauri 2, serde), React 18 (TypeScript), Zustand 4, Tailwind CSS 3, Vitest + @testing-library/react.

## Global Constraints

- Rust: edition 2021, rusqlite 0.31 bundled, serde with `derive` feature, Tauri 2
- TypeScript: strict mode, no `any`
- Tailwind: use existing dark-mode classes (`dark:`) consistent with the rest of the UI
- All dates in SQLite stored as `TEXT` — use `datetime('now', '-7 days')` for the 7-day window
- Subtasks excluded from stats (`parent_task_id IS NULL`)
- Time formatted via existing `formatTotal` from `src/lib/timeUtils.ts`
- No new Zustand stores — stats live in local `useState` inside `Dashboard.tsx`
- Run tests with: Rust → `cd src-tauri && cargo test`; Frontend → `npm test` (Vitest, from repo root)

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Create | `src-tauri/src/commands/stats.rs` | `get_productivity_stats` command + inner helper + Rust tests |
| Modify | `src-tauri/src/commands/mod.rs` | expose `pub mod stats` |
| Modify | `src-tauri/src/lib.rs` | register command in `invoke_handler` |
| Modify | `src/types.ts` | add `ProductivityStats` interface |
| Modify | `src/lib/tauri.ts` | add `getProductivityStats` to `api` object |
| Create | `src/components/ProductivityPanel.tsx` | stat tiles UI component |
| Create | `src/components/__tests__/ProductivityPanel.test.tsx` | component tests |
| Modify | `src/pages/Dashboard.tsx` | fetch stats + render panel |

---

## Task 1: Rust command `get_productivity_stats`

**Files:**
- Create: `src-tauri/src/commands/stats.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `commands::stats::get_productivity_stats` (registered Tauri command)
- Produces: `pub struct ProductivityStats { tasks_completed_week: i64, total_seconds_week: i64, on_time_count: i64, late_count: i64 }`

- [ ] **Step 1: Write the failing Rust tests**

Create `src-tauri/src/commands/stats.rs` with the test module only (no implementation yet):

```rust
use crate::db::DbState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ProductivityStats {
    pub tasks_completed_week: i64,
    pub total_seconds_week: i64,
    pub on_time_count: i64,
    pub late_count: i64,
}

pub fn get_stats_from_conn(conn: &rusqlite::Connection) -> Result<ProductivityStats, String> {
    todo!()
}

#[tauri::command]
pub fn get_productivity_stats(state: State<DbState>) -> Result<ProductivityStats, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    get_stats_from_conn(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn setup() -> (rusqlite::Connection, i64) {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('Test', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        (conn, list_id)
    }

    #[test]
    fn test_stats_empty_db() {
        let (conn, _) = setup();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.tasks_completed_week, 0);
        assert_eq!(stats.total_seconds_week, 0);
        assert_eq!(stats.on_time_count, 0);
        assert_eq!(stats.late_count, 0);
    }

    #[test]
    fn test_counts_completed_tasks_this_week() {
        let (conn, list_id) = setup();
        // Insert a task completed now (within 7 days)
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, status)
             VALUES (?1, 'Done task', 'normal', 0, 1, datetime('now'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        // Insert a task completed 10 days ago (outside window)
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, status)
             VALUES (?1, 'Old task', 'normal', 1, 1, datetime('now', '-10 days'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.tasks_completed_week, 1);
    }

    #[test]
    fn test_excludes_subtasks_from_count() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        // Subtask completed this week
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, is_subtask, title, priority, position, completed, completed_at, status)
             VALUES (?1, ?2, 1, 'Sub', 'normal', 0, 1, datetime('now'), 'done')",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.tasks_completed_week, 0);
    }

    #[test]
    fn test_sums_timer_seconds_this_week() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'T', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let task_id = conn.last_insert_rowid();
        // Session this week: 3600 seconds
        conn.execute(
            "INSERT INTO timer_sessions (task_id, started_at, stopped_at, duration_seconds)
             VALUES (?1, datetime('now', '-1 day'), datetime('now'), 3600)",
            rusqlite::params![task_id],
        ).unwrap();
        // Session outside window: 7200 seconds
        conn.execute(
            "INSERT INTO timer_sessions (task_id, started_at, stopped_at, duration_seconds)
             VALUES (?1, datetime('now', '-10 days'), datetime('now', '-10 days', '+2 hours'), 7200)",
            rusqlite::params![task_id],
        ).unwrap();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.total_seconds_week, 3600);
    }

    #[test]
    fn test_on_time_vs_late() {
        let (conn, list_id) = setup();
        // On-time: completed today, due today
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, due_date, status)
             VALUES (?1, 'On time', 'normal', 0, 1, datetime('now'), date('now'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        // Late: completed today, due yesterday
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, due_date, status)
             VALUES (?1, 'Late', 'normal', 1, 1, datetime('now'), date('now', '-1 day'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        // No due date: excluded from both counts
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, status)
             VALUES (?1, 'No date', 'normal', 2, 1, datetime('now'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.on_time_count, 1);
        assert_eq!(stats.late_count, 1);
    }
}
```

- [ ] **Step 2: Register the module so tests compile**

In `src-tauri/src/commands/mod.rs`, add:
```rust
pub mod stats;
```
(Place after the existing `pub mod settings;` line.)

- [ ] **Step 3: Run tests to confirm they fail**

```powershell
cd src-tauri && cargo test stats
```
Expected: compile succeeds, tests panic with `todo!()`.

- [ ] **Step 4: Implement `get_stats_from_conn`**

Replace the `todo!()` in `stats.rs`:

```rust
pub fn get_stats_from_conn(conn: &rusqlite::Connection) -> Result<ProductivityStats, String> {
    let tasks_completed_week: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks
         WHERE completed = 1
           AND completed_at >= datetime('now', '-7 days')
           AND parent_task_id IS NULL",
        [],
        |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    let total_seconds_week: i64 = conn.query_row(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM timer_sessions
         WHERE started_at >= datetime('now', '-7 days')",
        [],
        |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    let (on_time_count, late_count): (i64, i64) = conn.query_row(
        "SELECT
           COALESCE(SUM(CASE WHEN date(completed_at) <= due_date THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN date(completed_at) > due_date  THEN 1 ELSE 0 END), 0)
         FROM tasks
         WHERE completed = 1
           AND completed_at >= datetime('now', '-7 days')
           AND due_date IS NOT NULL
           AND parent_task_id IS NULL",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|e| e.to_string())?;

    Ok(ProductivityStats { tasks_completed_week, total_seconds_week, on_time_count, late_count })
}
```

- [ ] **Step 5: Register command in `lib.rs`**

In `src-tauri/src/lib.rs`, add to the `invoke_handler!` macro list:
```rust
commands::stats::get_productivity_stats,
```
(Place after `commands::settings::change_data_dir,`.)

- [ ] **Step 6: Run Rust tests to verify they pass**

```powershell
cd src-tauri && cargo test stats
```
Expected: all 5 stats tests pass.

- [ ] **Step 7: Run full Rust test suite to check for regressions**

```powershell
cd src-tauri && cargo test
```
Expected: all tests pass.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/commands/stats.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add get_productivity_stats Rust command with weekly stats"
```

---

## Task 2: TypeScript types and invoke wrapper

**Files:**
- Modify: `src/types.ts`
- Modify: `src/lib/tauri.ts`

**Interfaces:**
- Consumes: Rust struct `ProductivityStats` (field names snake_case, mapped by Tauri's serde)
- Produces: `ProductivityStats` TypeScript interface; `api.getProductivityStats()` returning `Promise<ProductivityStats>`

- [ ] **Step 1: Add `ProductivityStats` to `src/types.ts`**

After the `DashboardData` interface (around line 38), add:
```typescript
export interface ProductivityStats {
  tasks_completed_week: number
  total_seconds_week: number
  on_time_count: number
  late_count: number
}
```

- [ ] **Step 2: Add import and method to `src/lib/tauri.ts`**

At the top of the file, add `ProductivityStats` to the import:
```typescript
import type {
  List, Task, Tag, DashboardData, TimerSession, ActiveTimer,
  Settings, StartTimerResult, ProductivityStats,
} from '../types'
```

Then add to the `api` object, after `getDashboardTasks`:
```typescript
getProductivityStats: () => invoke<ProductivityStats>('get_productivity_stats'),
```

- [ ] **Step 3: Verify TypeScript compiles**

```powershell
npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 4: Commit**

```powershell
git add src/types.ts src/lib/tauri.ts
git commit -m "feat: add ProductivityStats type and getProductivityStats invoke wrapper"
```

---

## Task 3: `ProductivityPanel` component

**Files:**
- Create: `src/components/ProductivityPanel.tsx`
- Create: `src/components/__tests__/ProductivityPanel.test.tsx`

**Interfaces:**
- Consumes: `ProductivityStats` from `src/types.ts`; `formatTotal` from `src/lib/timeUtils.ts`
- Produces: `<ProductivityPanel stats={ProductivityStats} />` default export

- [ ] **Step 1: Write the failing component tests**

Create `src/components/__tests__/ProductivityPanel.test.tsx`:

```tsx
import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import ProductivityPanel from '../ProductivityPanel'
import type { ProductivityStats } from '../../types'

const baseStats: ProductivityStats = {
  tasks_completed_week: 0,
  total_seconds_week: 0,
  on_time_count: 0,
  late_count: 0,
}

describe('ProductivityPanel', () => {
  it('renders the "Ten tydzień" heading', () => {
    render(<ProductivityPanel stats={baseStats} />)
    expect(screen.getByText('Ten tydzień')).toBeTruthy()
  })

  it('shows completed task count', () => {
    const stats = { ...baseStats, tasks_completed_week: 7 }
    render(<ProductivityPanel stats={stats} />)
    expect(screen.getByText('7')).toBeTruthy()
  })

  it('shows formatted time for non-zero seconds', () => {
    const stats = { ...baseStats, total_seconds_week: 5400 }
    render(<ProductivityPanel stats={stats} />)
    expect(screen.getByText('1h 30m')).toBeTruthy()
  })

  it('shows 0m when total_seconds_week is 0', () => {
    render(<ProductivityPanel stats={baseStats} />)
    expect(screen.getByText('0m')).toBeTruthy()
  })

  it('shows — for terminowość when no tasks with due dates completed', () => {
    render(<ProductivityPanel stats={baseStats} />)
    expect(screen.getByText('—')).toBeTruthy()
  })

  it('shows on_time / total for terminowość when data exists', () => {
    const stats = { ...baseStats, on_time_count: 3, late_count: 1 }
    render(<ProductivityPanel stats={stats} />)
    expect(screen.getByText('3 / 4')).toBeTruthy()
  })
})
```

- [ ] **Step 2: Run tests to verify they fail**

```powershell
npm test -- ProductivityPanel
```
Expected: FAIL — component file not found.

- [ ] **Step 3: Implement `ProductivityPanel`**

Create `src/components/ProductivityPanel.tsx`:

```tsx
import type { ProductivityStats } from '../types'
import { formatTotal } from '../lib/timeUtils'

interface Props {
  stats: ProductivityStats
}

export default function ProductivityPanel({ stats }: Props) {
  const { tasks_completed_week, total_seconds_week, on_time_count, late_count } = stats
  const totalWithDate = on_time_count + late_count
  const onTimeDisplay = totalWithDate === 0 ? '—' : `${on_time_count} / ${totalWithDate}`
  const timeDisplay = formatTotal(total_seconds_week) || '0m'

  return (
    <div className="w-56 border-l border-gray-100 dark:border-gray-800 flex flex-col flex-shrink-0">
      <div className="px-4 py-4 border-b border-gray-100 dark:border-gray-800">
        <h3 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide">
          Ten tydzień
        </h3>
      </div>
      <div className="flex flex-col gap-1 px-4 py-4">
        <StatTile value={String(tasks_completed_week)} label="ukończone zadania" />
        <StatTile value={timeDisplay} label="zarejestrowany czas" />
        <StatTile value={onTimeDisplay} label="ukończonych na czas" />
      </div>
    </div>
  )
}

function StatTile({ value, label }: { value: string; label: string }) {
  return (
    <div className="py-3 border-b border-gray-100 dark:border-gray-800 last:border-0">
      <div className="text-2xl font-bold text-gray-900 dark:text-gray-100">{value}</div>
      <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{label}</div>
    </div>
  )
}
```

- [ ] **Step 4: Run tests to verify they pass**

```powershell
npm test -- ProductivityPanel
```
Expected: all 6 tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src/components/ProductivityPanel.tsx src/components/__tests__/ProductivityPanel.test.tsx
git commit -m "feat: add ProductivityPanel component with weekly stat tiles"
```

---

## Task 4: Wire `ProductivityPanel` into `Dashboard`

**Files:**
- Modify: `src/pages/Dashboard.tsx`

**Interfaces:**
- Consumes: `api.getProductivityStats()` from `src/lib/tauri.ts`; `<ProductivityPanel>` from `src/components/ProductivityPanel.tsx`; `ProductivityStats` from `src/types.ts`

- [ ] **Step 1: Update `Dashboard.tsx`**

Replace the full content of `src/pages/Dashboard.tsx` with:

```tsx
import { useEffect, useState } from 'react'
import type { Task } from '../types'
import type { ProductivityStats } from '../types'
import type { View } from '../App'
import { useTasksStore } from '../store/tasksStore'
import { api } from '../lib/tauri'
import TaskCard from '../components/TaskCard'
import TaskEditor from '../components/TaskEditor'
import ProductivityPanel from '../components/ProductivityPanel'

interface Props {
  onNavigate: (v: View) => void
}

export default function Dashboard({ onNavigate: _onNavigate }: Props) {
  const { dashboard, loadDashboard, update } = useTasksStore()
  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [stats, setStats] = useState<ProductivityStats | null>(null)
  const [statsError, setStatsError] = useState(false)

  useEffect(() => {
    loadDashboard()
    api.getProductivityStats()
      .then(setStats)
      .catch(() => setStatsError(true))
  }, [loadDashboard])

  async function handleToggleComplete(task: Task) {
    await update(task.id, task.list_id, { completed: !task.completed })
    loadDashboard()
    api.getProductivityStats().then(setStats).catch(() => {})
  }

  function handleOpenTask(task: Task) {
    setSelectedTask(task)
  }

  if (!dashboard) {
    return (
      <div className="flex items-center justify-center h-full text-gray-400 text-sm">
        Loading…
      </div>
    )
  }

  return (
    <div className="flex h-full">
      <div className="flex-1 overflow-y-auto">
        <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-800">
          <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100">Dashboard</h2>
        </div>

        <Section
          title="Due Today / Overdue"
          tasks={dashboard.overdue}
          emptyMessage="No overdue or due-today tasks."
          onOpen={handleOpenTask}
          onToggleComplete={handleToggleComplete}
        />

        <Section
          title="High Priority"
          tasks={dashboard.high_priority}
          emptyMessage="No high-priority tasks."
          onOpen={handleOpenTask}
          onToggleComplete={handleToggleComplete}
        />

        <Section
          title="Upcoming"
          tasks={dashboard.upcoming}
          emptyMessage="No upcoming tasks in the next 7 days."
          onOpen={handleOpenTask}
          onToggleComplete={handleToggleComplete}
        />
      </div>

      {stats && <ProductivityPanel stats={stats} />}
      {statsError && (
        <div className="w-56 border-l border-gray-100 dark:border-gray-800 px-4 py-4 text-xs text-gray-400">
          Nie udało się załadować statystyk.
        </div>
      )}

      {selectedTask && (
        <TaskEditor
          task={selectedTask}
          listId={selectedTask.list_id}
          onClose={() => setSelectedTask(null)}
        />
      )}
    </div>
  )
}

interface SectionProps {
  title: string
  tasks: Task[]
  emptyMessage: string
  onOpen: (task: Task) => void
  onToggleComplete: (task: Task) => void
}

function Section({ title, tasks, emptyMessage, onOpen, onToggleComplete }: SectionProps) {
  return (
    <div className="mb-6">
      <div className="px-6 py-2 border-b border-gray-100 dark:border-gray-800">
        <h3 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide">
          {title}
          {tasks.length > 0 && (
            <span className="ml-2 text-xs font-normal text-gray-400">({tasks.length})</span>
          )}
        </h3>
      </div>
      {tasks.length === 0 ? (
        <p className="px-6 py-3 text-sm text-gray-400 dark:text-gray-500">{emptyMessage}</p>
      ) : (
        tasks.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            onOpen={onOpen}
            onToggleComplete={onToggleComplete}
          />
        ))
      )}
    </div>
  )
}
```

- [ ] **Step 2: Verify TypeScript compiles**

```powershell
npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Run the full frontend test suite**

```powershell
npm test
```
Expected: all tests pass (no regressions in TaskCard, MarkdownRenderer, or timeUtils tests).

- [ ] **Step 4: Run the full Rust test suite**

```powershell
cd src-tauri && cargo test
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src/pages/Dashboard.tsx
git commit -m "feat: wire ProductivityPanel into Dashboard with weekly stats"
```
