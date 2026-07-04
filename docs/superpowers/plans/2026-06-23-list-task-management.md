# List & Task Management Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move list deletion to the list view header, add inline list name editing, add task deletion from the list view, and add subtasks inside the task editor.

**Architecture:** The Rust backend gets a new `parent_task_id` column migration plus two new commands (`create_subtask`, `get_subtasks`). The TypeScript layer adds the new API calls. Four UI components change: Sidebar loses its delete button, ListDetail header gains delete+inline-rename, TaskCard gains a hover delete button, TaskEditor gains a subtask section with local state.

**Tech Stack:** Tauri 2 (Rust/rusqlite), React 18 (TypeScript), Zustand 4, Tailwind CSS 3

---

## File Map

| File | Change |
|---|---|
| `src-tauri/src/db.rs` | Add `ALTER TABLE tasks ADD COLUMN parent_task_id` migration |
| `src-tauri/src/commands/tasks.rs` | Add `parent_task_id` to `Task` struct, update all SELECTs to col 11, filter top-level in `get_tasks`+`get_dashboard_tasks`, add `create_subtask`+`get_subtasks` commands |
| `src-tauri/src/lib.rs` | Register `create_subtask` and `get_subtasks` |
| `src/types.ts` | Add `parent_task_id: number \| null` to `Task` |
| `src/lib/tauri.ts` | Add `createSubtask`, `getSubtasks` |
| `src/components/Sidebar.tsx` | Remove delete button + handler, simplify list items back to plain `<button>` |
| `src/pages/ListDetail.tsx` | Add inline title edit, delete list button in header, wire `onDelete` to TaskCard |
| `src/components/TaskCard.tsx` | Add optional `onDelete` prop + hover `×` button |
| `src/components/TaskEditor.tsx` | Add subtasks section: list, add input, toggle, delete |

---

### Task 1: DB migration — add `parent_task_id` column

**Files:**
- Modify: `src-tauri/src/db.rs`

- [ ] **Step 1: Add the migration statement**

The existing migration uses `CREATE TABLE IF NOT EXISTS` which won't add a new column to an existing table. Add an `ALTER TABLE` after the batch — SQLite ignores "duplicate column" errors gracefully via `.ok()`.

Replace the `migrate` function body in `src-tauri/src/db.rs`:

```rust
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
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            list_id         INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
            parent_task_id  INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
            title           TEXT NOT NULL,
            description     TEXT,
            priority        TEXT NOT NULL DEFAULT 'normal' CHECK(priority IN ('normal','high')),
            due_date        TEXT,
            completed       INTEGER NOT NULL DEFAULT 0,
            completed_at    TEXT,
            position        INTEGER NOT NULL DEFAULT 0,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
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
    ")?;
    // Idempotent column add for existing databases — fails silently if column exists
    conn.execute(
        "ALTER TABLE tasks ADD COLUMN parent_task_id INTEGER REFERENCES tasks(id) ON DELETE CASCADE",
        [],
    ).ok();
    Ok(())
}
```

- [ ] **Step 2: Verify migration test still passes**

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cd src-tauri
cargo test db::tests -- --nocapture
```

Expected: `test db::tests::test_migrations_create_all_tables ... ok` and `test db::tests::test_foreign_keys_enabled ... ok`

- [ ] **Step 3: Commit**

```powershell
git add src-tauri/src/db.rs
git commit -m "feat(db): add parent_task_id column migration for subtasks"
```

---

### Task 2: Rust — update Task struct and commands

**Files:**
- Modify: `src-tauri/src/commands/tasks.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write tests for the new commands**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src-tauri/src/commands/tasks.rs`:

```rust
    #[test]
    fn test_create_subtask() {
        let (conn, list_id) = setup();
        // Create parent task
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        // Create subtask
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, title, priority, position) VALUES (?1, ?2, 'Sub', 'normal', 0)",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        let subtask_parent: i64 = conn.query_row(
            "SELECT parent_task_id FROM tasks WHERE title='Sub'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(subtask_parent, parent_id);
    }

    #[test]
    fn test_delete_parent_cascades_subtasks() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, title, priority, position) VALUES (?1, ?2, 'Sub', 'normal', 0)",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        conn.execute("DELETE FROM tasks WHERE id=?1", rusqlite::params![parent_id]).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE parent_task_id=?1",
            rusqlite::params![parent_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_get_tasks_excludes_subtasks() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, title, priority, position) VALUES (?1, ?2, 'Sub', 'normal', 0)",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE list_id=?1 AND parent_task_id IS NULL",
            rusqlite::params![list_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }
```

- [ ] **Step 2: Run tests to confirm they pass (DB already updated)**

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cd src-tauri
cargo test commands::tasks::tests -- --nocapture
```

Expected: All 5 tests pass (2 existing + 3 new).

- [ ] **Step 3: Update `Task` struct to add `parent_task_id`**

In `src-tauri/src/commands/tasks.rs`, replace the `Task` struct:

```rust
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
    pub parent_task_id: Option<i64>,
    pub tags: Vec<Tag>,
    pub total_seconds: i64,
}
```

- [ ] **Step 4: Update `row_to_task` — add `parent_task_id` at column index 11**

Replace the `row_to_task` function:

```rust
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
        parent_task_id: row.get(11)?,
        tags,
        total_seconds,
    })
}
```

- [ ] **Step 5: Update all SELECT queries that use `row_to_task` to include `parent_task_id`**

Every query that calls `row_to_task` must select 12 columns (indices 0-11). Update each one:

**`get_tasks`** — also filter to top-level only:
```rust
#[tauri::command]
pub fn get_tasks(state: State<DbState>, list_id: i64) -> Result<Vec<Task>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                    position,created_at,updated_at,parent_task_id
             FROM tasks WHERE list_id=?1 AND parent_task_id IS NULL
             ORDER BY completed ASC, position ASC",
        )
        .map_err(|e| e.to_string())?;
    let tasks: Vec<Task> = stmt
        .query_map(rusqlite::params![list_id], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}
```

**`create_task`** — update the final SELECT:
```rust
    conn.query_row(
        "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                position,created_at,updated_at,parent_task_id FROM tasks WHERE id=?1",
        rusqlite::params![id],
        |row| row_to_task(row, &conn),
    )
    .map_err(|e| e.to_string())
```

**`update_task`** — update the final SELECT:
```rust
    conn.query_row(
        "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                position,created_at,updated_at,parent_task_id FROM tasks WHERE id=?1",
        rusqlite::params![id],
        |row| row_to_task(row, &conn),
    )
    .map_err(|e| e.to_string())
```

**`get_dashboard_tasks`** — update `base` constant and add `parent_task_id IS NULL`:
```rust
    let base = "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                        position,created_at,updated_at,parent_task_id
                FROM tasks WHERE completed=0 AND parent_task_id IS NULL";
```

- [ ] **Step 6: Add `create_subtask` and `get_subtasks` commands**

Add these two functions before the `#[cfg(test)]` block:

```rust
#[tauri::command]
pub fn create_subtask(
    state: State<DbState>,
    parent_task_id: i64,
    title: String,
) -> Result<Task, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let list_id: i64 = conn
        .query_row(
            "SELECT list_id FROM tasks WHERE id = ?1",
            rusqlite::params![parent_task_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE parent_task_id = ?1",
            rusqlite::params![parent_task_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO tasks (list_id, parent_task_id, title, priority, position)
         VALUES (?1, ?2, ?3, 'normal', ?4)",
        rusqlite::params![list_id, parent_task_id, title, position],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                position,created_at,updated_at,parent_task_id FROM tasks WHERE id=?1",
        rusqlite::params![id],
        |row| row_to_task(row, &conn),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_subtasks(
    state: State<DbState>,
    task_id: i64,
) -> Result<Vec<Task>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                    position,created_at,updated_at,parent_task_id
             FROM tasks WHERE parent_task_id=?1 ORDER BY position ASC",
        )
        .map_err(|e| e.to_string())?;
    let tasks: Vec<Task> = stmt
        .query_map(rusqlite::params![task_id], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}
```

- [ ] **Step 7: Register new commands in `src-tauri/src/lib.rs`**

Add to the `invoke_handler!` macro (after `get_all_tags`):

```rust
            commands::tasks::create_subtask,
            commands::tasks::get_subtasks,
```

- [ ] **Step 8: Run all Rust tests**

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cd src-tauri
cargo test -- --nocapture
```

Expected: All tests pass.

- [ ] **Step 9: Commit**

```powershell
git add src-tauri/src/commands/tasks.rs src-tauri/src/lib.rs
git commit -m "feat(rust): add parent_task_id to Task, create_subtask and get_subtasks commands"
```

---

### Task 3: TypeScript types + API layer

**Files:**
- Modify: `src/types.ts`
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Add `parent_task_id` to Task interface in `src/types.ts`**

Replace the `Task` interface:

```typescript
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
  parent_task_id: number | null
  tags: Tag[]
  total_seconds: number
}
```

- [ ] **Step 2: Add `createSubtask` and `getSubtasks` to `src/lib/tauri.ts`**

Add after the `deleteTask` line:

```typescript
  createSubtask: (parentTaskId: number, title: string) =>
    invoke<Task>('create_subtask', { parentTaskId, title }),
  getSubtasks: (taskId: number) =>
    invoke<Task[]>('get_subtasks', { taskId }),
```

- [ ] **Step 3: Verify TypeScript compiles**

```powershell
npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 4: Commit**

```powershell
git add src/types.ts src/lib/tauri.ts
git commit -m "feat(ts): add parent_task_id to Task type, createSubtask and getSubtasks API"
```

---

### Task 4: Sidebar — remove delete button

**Files:**
- Modify: `src/components/Sidebar.tsx`

The sidebar currently wraps each list item in a `<div className="relative group/item">` with a hover-visible `×` delete button. Remove all of that — deletion now lives in the list view header.

- [ ] **Step 1: Replace the list section in `src/components/Sidebar.tsx`**

Remove `remove` from the destructured store. Remove `handleDeleteList`. Replace the list items block (lines 53-74) with a simpler version:

```tsx
import { useState } from 'react'
import type { View } from '../App'
import { useListsStore } from '../store/listsStore'

interface Props {
  view: View
  onNavigate: (v: View) => void
}

export default function Sidebar({ view, onNavigate }: Props) {
  const { lists, create } = useListsStore()

  const [adding, setAdding] = useState(false)
  const [newTitle, setNewTitle] = useState('')

  async function handleAddList() {
    const title = newTitle.trim()
    if (!title) { setAdding(false); return }
    const list = await create(title)
    setNewTitle('')
    setAdding(false)
    onNavigate({ type: 'list', id: list.id })
  }

  const navItem = (active: boolean) =>
    `flex items-center gap-2 px-3 py-2 rounded-lg text-sm cursor-pointer transition-colors ${
      active
        ? 'bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300 font-medium'
        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'
    }`

  return (
    <aside className="w-60 shrink-0 border-r border-gray-200 dark:border-gray-700 flex flex-col h-screen p-3 gap-1">
      <h1 className="text-lg font-bold px-2 py-3 text-gray-900 dark:text-gray-100">Smart Todo</h1>

      <button
        className={navItem(view.type === 'dashboard')}
        onClick={() => onNavigate({ type: 'dashboard' })}
      >
        <span aria-hidden="true">📋</span> Dashboard
      </button>

      <hr className="border-gray-200 dark:border-gray-700 my-1" />

      <div className="flex-1 overflow-y-auto space-y-0.5">
        {lists.map((list) => (
          <button
            key={list.id}
            className={`${navItem(view.type === 'list' && view.id === list.id)} w-full`}
            onClick={() => onNavigate({ type: 'list', id: list.id })}
          >
            <span
              className="w-2.5 h-2.5 rounded-full shrink-0"
              style={{ backgroundColor: list.color ?? '#6366f1' }}
            />
            <span className="truncate flex-1 text-left">{list.title}</span>
          </button>
        ))}

        {adding ? (
          <form
            onSubmit={(e) => { e.preventDefault(); handleAddList() }}
            className="flex gap-1 mt-1"
          >
            <input
              autoFocus
              className="flex-1 text-sm px-2 py-1 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none"
              aria-label="List name"
              placeholder="List name…"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onBlur={handleAddList}
              onKeyDown={(e) => e.key === 'Escape' && setAdding(false)}
            />
          </form>
        ) : (
          <button
            className="flex items-center gap-2 px-3 py-2 text-sm text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 w-full rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            onClick={() => setAdding(true)}
          >
            <span>+</span> New List
          </button>
        )}
      </div>

      <hr className="border-gray-200 dark:border-gray-700 my-1" />

      <button
        className={navItem(view.type === 'settings')}
        onClick={() => onNavigate({ type: 'settings' })}
      >
        <span aria-hidden="true">⚙</span> Settings
      </button>
    </aside>
  )
}
```

- [ ] **Step 2: Verify no TypeScript errors**

```powershell
npx tsc --noEmit
```

- [ ] **Step 3: Commit**

```powershell
git add src/components/Sidebar.tsx
git commit -m "feat(ui): remove delete button from sidebar"
```

---

### Task 5: ListDetail — delete list + inline rename

**Files:**
- Modify: `src/pages/ListDetail.tsx`

The header currently shows just the list title. Replace it with:
- Clicking the title → inline text input (auto-focused), blur/Enter saves via `useListsStore().update()`
- A delete button (red trash/×) right of the title, with `window.confirm()`, calls `useListsStore().remove()` then `onNavigate({ type: 'dashboard' })`

- [ ] **Step 1: Replace `src/pages/ListDetail.tsx`**

```tsx
import { useEffect, useState, useRef } from 'react'
import type { Task } from '../types'
import type { View } from '../App'
import { useListsStore } from '../store/listsStore'
import { useTasksStore } from '../store/tasksStore'
import TaskCard from '../components/TaskCard'
import TaskEditor from '../components/TaskEditor'

interface Props {
  listId: number
  onNavigate: (v: View) => void
}

export default function ListDetail({ listId, onNavigate }: Props) {
  const lists = useListsStore((s) => s.lists)
  const { update: updateList, remove: removeList } = useListsStore()
  const { tasks, loadList, create, update, remove } = useTasksStore()
  const list = lists.find((l) => l.id === listId)
  const listTasks = tasks[listId] ?? []

  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [showCompleted, setShowCompleted] = useState(false)
  const [addingTitle, setAddingTitle] = useState('')
  const [isAdding, setIsAdding] = useState(false)

  // Inline rename state
  const [editingTitle, setEditingTitle] = useState(false)
  const [titleDraft, setTitleDraft] = useState('')
  const titleInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    loadList(listId)
  }, [listId, loadList])

  // Keep title draft in sync when list changes (e.g. navigating to different list)
  useEffect(() => {
    if (list) setTitleDraft(list.title)
  }, [list?.id])

  const incomplete = listTasks.filter((t) => !t.completed)
  const completed = listTasks.filter((t) => t.completed)

  async function handleAdd() {
    const title = addingTitle.trim()
    if (!title) { setIsAdding(false); return }
    const task = await create(listId, title)
    setAddingTitle('')
    setIsAdding(false)
    setSelectedTask(task)
  }

  async function handleToggleComplete(task: Task) {
    await update(task.id, listId, { completed: !task.completed })
  }

  async function handleDeleteTask(task: Task) {
    if (!window.confirm(`Delete "${task.title}"?`)) return
    if (selectedTask?.id === task.id) setSelectedTask(null)
    await remove(task.id, listId)
  }

  function handleStartEditTitle() {
    setTitleDraft(list?.title ?? '')
    setEditingTitle(true)
    setTimeout(() => titleInputRef.current?.select(), 0)
  }

  async function handleSaveTitle() {
    setEditingTitle(false)
    const trimmed = titleDraft.trim()
    if (trimmed && trimmed !== list?.title) {
      await updateList(listId, trimmed)
    }
  }

  async function handleDeleteList() {
    if (!window.confirm(`Delete list "${list?.title}" and all its tasks?`)) return
    await removeList(listId)
    onNavigate({ type: 'dashboard' })
  }

  return (
    <div className="flex h-full">
      {/* Task list */}
      <div className="flex-1 overflow-y-auto">
        {/* Header */}
        <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-800 flex items-center gap-3">
          {list?.color && (
            <span
              className="w-3 h-3 rounded-full shrink-0"
              style={{ backgroundColor: list.color }}
            />
          )}

          {editingTitle ? (
            <input
              ref={titleInputRef}
              className="text-xl font-semibold bg-transparent border-b border-indigo-400 outline-none text-gray-900 dark:text-gray-100 flex-1"
              value={titleDraft}
              onChange={(e) => setTitleDraft(e.target.value)}
              onBlur={handleSaveTitle}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleSaveTitle()
                if (e.key === 'Escape') setEditingTitle(false)
              }}
            />
          ) : (
            <h2
              className="text-xl font-semibold text-gray-900 dark:text-gray-100 cursor-pointer hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors flex-1"
              onClick={handleStartEditTitle}
              title="Click to rename"
            >
              {list?.title ?? 'List'}
            </h2>
          )}

          <button
            onClick={handleDeleteList}
            className="text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 rounded px-2 py-1 text-xs transition-colors shrink-0"
            title="Delete list"
          >
            Delete list
          </button>
        </div>

        {/* Incomplete tasks */}
        {incomplete.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            onOpen={setSelectedTask}
            onToggleComplete={handleToggleComplete}
            onDelete={handleDeleteTask}
          />
        ))}

        {/* Add task row */}
        {isAdding ? (
          <form
            onSubmit={(e) => { e.preventDefault(); handleAdd() }}
            className="px-4 py-2 border-b border-gray-100 dark:border-gray-800"
          >
            <input
              autoFocus
              className="w-full text-sm px-3 py-2 border border-indigo-300 dark:border-indigo-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none"
              placeholder="Task title…"
              value={addingTitle}
              onChange={(e) => setAddingTitle(e.target.value)}
              onBlur={handleAdd}
              onKeyDown={(e) => e.key === 'Escape' && setIsAdding(false)}
            />
          </form>
        ) : (
          <button
            className="w-full text-left px-4 py-3 text-sm text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800/50 border-b border-gray-100 dark:border-gray-800 transition-colors"
            onClick={() => setIsAdding(true)}
          >
            + Add task
          </button>
        )}

        {/* Completed tasks disclosure */}
        {completed.length > 0 && (
          <div className="mt-4">
            <button
              className="w-full text-left px-4 py-2 text-sm text-gray-500 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800/50 flex items-center gap-2"
              onClick={() => setShowCompleted((v) => !v)}
            >
              <span>{showCompleted ? '▾' : '▸'}</span>
              Completed ({completed.length})
            </button>
            {showCompleted && completed.map((task) => (
              <TaskCard
                key={task.id}
                task={task}
                onOpen={setSelectedTask}
                onToggleComplete={handleToggleComplete}
                onDelete={handleDeleteTask}
              />
            ))}
          </div>
        )}
      </div>

      {/* Derive live task from store so TaskEditor always sees fresh data */}
      {selectedTask && (() => {
        const liveTask = listTasks.find(t => t.id === selectedTask.id) ?? selectedTask
        return (
          <TaskEditor
            task={liveTask}
            listId={listId}
            onClose={() => setSelectedTask(null)}
          />
        )
      })()}
    </div>
  )
}
```

- [ ] **Step 2: Verify no TypeScript errors**

```powershell
npx tsc --noEmit
```

- [ ] **Step 3: Commit**

```powershell
git add src/pages/ListDetail.tsx
git commit -m "feat(ui): add delete list + inline rename to list view header"
```

---

### Task 6: TaskCard — hover delete button

**Files:**
- Modify: `src/components/TaskCard.tsx`
- Modify: `src/components/__tests__/TaskCard.test.tsx`

- [ ] **Step 1: Write failing tests for the delete button**

Add to `src/components/__tests__/TaskCard.test.tsx`:

```tsx
  it('renders delete button when onDelete is provided', () => {
    const onDelete = vi.fn()
    render(<TaskCard task={mockTask} onOpen={vi.fn()} onToggleComplete={vi.fn()} onDelete={onDelete} />)
    expect(screen.getByRole('button', { name: /Delete task/i })).toBeInTheDocument()
  })

  it('does not render delete button when onDelete is not provided', () => {
    render(<TaskCard task={mockTask} onOpen={vi.fn()} onToggleComplete={vi.fn()} />)
    expect(screen.queryByRole('button', { name: /Delete task/i })).not.toBeInTheDocument()
  })

  it('calls onDelete with the task when delete button is clicked', async () => {
    const onDelete = vi.fn()
    render(<TaskCard task={mockTask} onOpen={vi.fn()} onToggleComplete={vi.fn()} onDelete={onDelete} />)
    await userEvent.click(screen.getByRole('button', { name: /Delete task/i }))
    expect(onDelete).toHaveBeenCalledWith(mockTask)
  })

  it('delete button click does not open the task editor', async () => {
    const onOpen = vi.fn()
    const onDelete = vi.fn()
    render(<TaskCard task={mockTask} onOpen={onOpen} onToggleComplete={vi.fn()} onDelete={onDelete} />)
    await userEvent.click(screen.getByRole('button', { name: /Delete task/i }))
    expect(onOpen).not.toHaveBeenCalled()
  })
```

- [ ] **Step 2: Run tests to confirm they fail**

```powershell
npx vitest run src/components/__tests__/TaskCard.test.tsx
```

Expected: 4 new tests fail with "Unable to find role 'button' with name /Delete task/i".

- [ ] **Step 3: Update `src/components/TaskCard.tsx` to add optional delete button**

```tsx
import type { Task } from '../types'
import { isOverdue, isDueToday, formatDueDate } from '../lib/dateUtils'
import { formatTotal } from '../lib/timeUtils'
import TimerWidget from './TimerWidget'

interface Props {
  task: Task
  onOpen: (task: Task) => void
  onToggleComplete: (task: Task) => void
  onDelete?: (task: Task) => void
}

export default function TaskCard({ task, onOpen, onToggleComplete, onDelete }: Props) {
  const overdue = !task.completed && isOverdue(task.due_date)
  const dueToday = !task.completed && isDueToday(task.due_date)

  return (
    <div
      className="flex items-center gap-3 px-4 py-3 border-b border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800/50 cursor-pointer group"
      onClick={() => onOpen(task)}
    >
      {/* Checkbox */}
      <input
        type="checkbox"
        checked={task.completed}
        onClick={(e) => e.stopPropagation()}
        onChange={(e) => { e.stopPropagation(); onToggleComplete(task) }}
        className="w-4 h-4 rounded accent-indigo-600 cursor-pointer shrink-0"
      />

      {/* Main content */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          {task.priority === 'high' && (
            <span className="text-red-500 text-xs shrink-0" title="High priority">⚑</span>
          )}
          <span className={`text-sm font-medium truncate ${task.completed ? 'line-through text-gray-400 dark:text-gray-500' : 'text-gray-900 dark:text-gray-100'}`}>
            {task.title}
          </span>
        </div>

        {/* Tags */}
        {task.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-1">
            {task.tags.map((tag) => (
              <span
                key={tag.id}
                className="text-xs px-1.5 py-0.5 rounded bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300"
              >
                {tag.name}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Right side: due date, total time, timer, delete */}
      <div className="flex items-center gap-2 shrink-0">
        {task.due_date && (
          <span className={`text-xs px-1.5 py-0.5 rounded ${
            overdue || dueToday
              ? 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400 font-medium'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400'
          }`}>
            {formatDueDate(task.due_date)}
          </span>
        )}
        {task.total_seconds > 0 && (
          <span className="text-xs text-gray-400 dark:text-gray-500">
            {formatTotal(task.total_seconds)}
          </span>
        )}
        {!task.completed && <TimerWidget taskId={task.id} />}
        {onDelete && (
          <button
            onClick={(e) => { e.stopPropagation(); onDelete(task) }}
            className="opacity-0 group-hover:opacity-100 text-gray-400 hover:text-red-500 transition-opacity text-base leading-none w-5 h-5 flex items-center justify-center rounded hover:bg-red-50 dark:hover:bg-red-900/20"
            aria-label={`Delete task ${task.title}`}
          >
            ×
          </button>
        )}
      </div>
    </div>
  )
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```powershell
npx vitest run src/components/__tests__/TaskCard.test.tsx
```

Expected: All tests pass (existing 8 + new 4 = 12 total).

- [ ] **Step 5: Commit**

```powershell
git add src/components/TaskCard.tsx src/components/__tests__/TaskCard.test.tsx
git commit -m "feat(ui): add optional hover delete button to TaskCard"
```

---

### Task 7: TaskEditor — subtasks section

**Files:**
- Modify: `src/components/TaskEditor.tsx`

Subtasks are managed with local React state in the editor (not in the Zustand store). On mount (and when `task.id` changes), load subtasks via `api.getSubtasks`. Create via `api.createSubtask`, toggle via `api.updateTask`, delete via `api.deleteTask`.

- [ ] **Step 1: Replace `src/components/TaskEditor.tsx`**

```tsx
import { useState, useEffect } from 'react'
import type { Task, TimerSession } from '../types'
import { useTasksStore } from '../store/tasksStore'
import { useTimerStore } from '../store/timerStore'
import { api } from '../lib/tauri'
import { formatTotal, formatLive } from '../lib/timeUtils'
import MarkdownRenderer from './MarkdownRenderer'
import TagInput from './TagInput'

interface Props {
  task: Task
  listId: number
  onClose: () => void
}

export default function TaskEditor({ task, listId, onClose }: Props) {
  const { update, remove, setTags } = useTasksStore()
  const { activeTaskId, elapsedSeconds, start, stop } = useTimerStore()
  const isTimerActive = activeTaskId === task.id

  const [title, setTitle] = useState(task.title)
  const [description, setDescription] = useState(task.description ?? '')
  const [preview, setPreview] = useState(false)
  const [sessions, setSessions] = useState<TimerSession[]>([])
  const [confirmDelete, setConfirmDelete] = useState(false)

  // Subtasks
  const [subtasks, setSubtasks] = useState<Task[]>([])
  const [newSubtask, setNewSubtask] = useState('')

  useEffect(() => {
    setTitle(task.title)
    setDescription(task.description ?? '')
    setPreview(false)
    setConfirmDelete(false)
    api.getTimerSessions(task.id).then(setSessions)
    api.getSubtasks(task.id).then(setSubtasks)
  }, [task.id])

  async function handleTitleBlur() {
    if (title.trim() && title !== task.title) {
      await update(task.id, listId, { title: title.trim() })
    }
  }

  async function handleDescriptionBlur() {
    if (description !== (task.description ?? '')) {
      await update(task.id, listId, { description })
    }
  }

  async function handlePriorityToggle() {
    const next = task.priority === 'high' ? 'normal' : 'high'
    await update(task.id, listId, { priority: next })
  }

  async function handleDueDateChange(e: React.ChangeEvent<HTMLInputElement>) {
    await update(task.id, listId, { dueDate: e.target.value })
  }

  async function handleTagsChange(tagNames: string[]) {
    await setTags(task.id, listId, tagNames)
  }

  async function handleTimerToggle() {
    if (isTimerActive) {
      await stop(task.id)
      const updated = await api.getTimerSessions(task.id)
      setSessions(updated)
    } else {
      await start(task.id)
    }
  }

  async function handleDelete() {
    await remove(task.id, listId)
    onClose()
  }

  async function handleAddSubtask() {
    const title = newSubtask.trim()
    if (!title) return
    const subtask = await api.createSubtask(task.id, title)
    setSubtasks((prev) => [...prev, subtask])
    setNewSubtask('')
  }

  async function handleToggleSubtask(subtask: Task) {
    const updated = await api.updateTask(subtask.id, { completed: !subtask.completed })
    setSubtasks((prev) => prev.map((s) => s.id === updated.id ? updated : s))
  }

  async function handleDeleteSubtask(subtask: Task) {
    await api.deleteTask(subtask.id)
    setSubtasks((prev) => prev.filter((s) => s.id !== subtask.id))
  }

  const totalSeconds = task.total_seconds + (isTimerActive ? elapsedSeconds : 0)

  return (
    <div className="w-80 shrink-0 border-l border-gray-200 dark:border-gray-700 flex flex-col h-full overflow-hidden bg-white dark:bg-gray-900">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-100 dark:border-gray-800">
        <span className="text-xs text-gray-400 uppercase tracking-wide">Task</span>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 text-xl leading-none"
          aria-label="Close"
        >
          ×
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
        {/* Title */}
        <input
          className="w-full text-base font-semibold bg-transparent border-b border-transparent hover:border-gray-200 dark:hover:border-gray-700 focus:border-indigo-400 dark:focus:border-indigo-500 outline-none text-gray-900 dark:text-gray-100 pb-1"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={handleTitleBlur}
        />

        {/* Priority */}
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={task.priority === 'high'}
            onChange={handlePriorityToggle}
            className="w-4 h-4 accent-red-500 cursor-pointer"
          />
          <span className={`text-sm ${task.priority === 'high' ? 'text-red-600 dark:text-red-400 font-medium' : 'text-gray-600 dark:text-gray-400'}`}>
            High priority
          </span>
        </label>

        {/* Due Date */}
        <div className="flex items-center gap-2">
          <span className="text-xs text-gray-500 w-20">Due date</span>
          <input
            type="date"
            value={task.due_date ?? ''}
            onChange={handleDueDateChange}
            className="text-xs border border-gray-200 dark:border-gray-700 rounded px-2 py-1 bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 outline-none"
          />
          {task.due_date && (
            <button
              onClick={() => update(task.id, listId, { dueDate: '' })}
              className="text-xs text-gray-400 hover:text-red-500"
              title="Clear due date"
            >
              ✕
            </button>
          )}
        </div>

        {/* Tags */}
        <div>
          <span className="text-xs text-gray-500 block mb-1">Tags</span>
          <TagInput tags={task.tags} onChange={handleTagsChange} />
        </div>

        {/* Description */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs text-gray-500">Description</span>
            <button
              onClick={() => setPreview((v) => !v)}
              className="text-xs text-indigo-500 hover:text-indigo-700"
            >
              {preview ? 'Edit' : 'Preview'}
            </button>
          </div>
          {preview ? (
            <div className="min-h-[80px] text-sm">
              {description ? (
                <MarkdownRenderer content={description} />
              ) : (
                <span className="text-gray-400 text-xs">No description</span>
              )}
            </div>
          ) : (
            <textarea
              className="w-full text-sm px-3 py-2 border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none resize-none min-h-[80px]"
              placeholder="Add a description (Markdown supported)…"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              onBlur={handleDescriptionBlur}
              rows={4}
            />
          )}
        </div>

        {/* Subtasks */}
        <div>
          <span className="text-xs text-gray-500 block mb-2">
            Subtasks
            {subtasks.length > 0 && (
              <span className="ml-1 text-gray-400">
                ({subtasks.filter((s) => s.completed).length}/{subtasks.length})
              </span>
            )}
          </span>
          {subtasks.map((s) => (
            <div key={s.id} className="flex items-center gap-2 py-1 group/subtask">
              <input
                type="checkbox"
                checked={s.completed}
                onChange={() => handleToggleSubtask(s)}
                className="w-3.5 h-3.5 accent-indigo-600 cursor-pointer shrink-0"
              />
              <span className={`text-sm flex-1 ${s.completed ? 'line-through text-gray-400 dark:text-gray-500' : 'text-gray-700 dark:text-gray-300'}`}>
                {s.title}
              </span>
              <button
                onClick={() => handleDeleteSubtask(s)}
                className="opacity-0 group-hover/subtask:opacity-100 text-gray-400 hover:text-red-500 transition-opacity text-base leading-none"
                aria-label={`Delete subtask ${s.title}`}
              >
                ×
              </button>
            </div>
          ))}
          <form
            onSubmit={(e) => { e.preventDefault(); handleAddSubtask() }}
            className="flex gap-1 mt-1"
          >
            <input
              className="flex-1 text-xs px-2 py-1 border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none"
              placeholder="Add subtask…"
              value={newSubtask}
              onChange={(e) => setNewSubtask(e.target.value)}
            />
            <button
              type="submit"
              className="text-xs px-2 py-1 bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300 rounded hover:bg-indigo-200 dark:hover:bg-indigo-800"
            >
              Add
            </button>
          </form>
        </div>

        {/* Timer */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs text-gray-500">Timer</span>
            <span className="text-xs text-gray-400">
              {totalSeconds > 0 ? `${formatTotal(totalSeconds)} total` : 'No time tracked'}
            </span>
          </div>
          <button
            onClick={handleTimerToggle}
            className={`flex items-center gap-2 text-sm px-3 py-2 rounded transition-colors w-full justify-center ${
              isTimerActive
                ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
            }`}
          >
            {isTimerActive ? (
              <>⏹ Stop — {formatLive(elapsedSeconds)}</>
            ) : (
              <>&#9654; Start timer</>
            )}
          </button>

          {sessions.length > 0 && (
            <div className="mt-2 space-y-1">
              {sessions.map((s) => (
                <div key={s.id} className="flex justify-between text-xs text-gray-400">
                  <span>{s.started_at.slice(0, 10)}</span>
                  <span>{s.duration_seconds ? formatTotal(s.duration_seconds) : '—'}</span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Delete */}
        <div className="pt-4 border-t border-gray-100 dark:border-gray-800">
          {confirmDelete ? (
            <div className="flex gap-2">
              <button
                onClick={handleDelete}
                className="flex-1 text-xs px-3 py-2 bg-red-500 text-white rounded hover:bg-red-600"
              >
                Confirm delete
              </button>
              <button
                onClick={() => setConfirmDelete(false)}
                className="flex-1 text-xs px-3 py-2 bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 rounded hover:bg-gray-200"
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              onClick={() => setConfirmDelete(true)}
              className="w-full text-xs px-3 py-2 text-red-500 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-900/20 rounded transition-colors"
            >
              Delete task
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Verify TypeScript compiles**

```powershell
npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 3: Run all unit tests**

```powershell
npm test
```

Expected: All 17+ tests pass.

- [ ] **Step 4: Commit**

```powershell
git add src/components/TaskEditor.tsx
git commit -m "feat(ui): add subtasks section to TaskEditor"
```

---

### Task 8: Update E2E Playwright mock + run full test suite

**Files:**
- Modify: `e2e/list-deletion.spec.ts` — update the mock to handle `create_subtask` and `get_subtasks`

- [ ] **Step 1: Update `installMock` in `e2e/list-deletion.spec.ts` to handle new commands**

In the `switch (cmd)` inside `window.__TAURI_INTERNALS__.invoke`, add before `default`:

```typescript
        case 'create_subtask': {
          const parentId = Number(args.parent_task_id)
          const parentTask = Object.values(db.tasks).flat().find(t => t.id === parentId)
          if (!parentTask) throw new Error('Parent task not found')
          const subtask = {
            id: nextId++,
            list_id: parentTask.list_id,
            parent_task_id: parentId,
            title: String(args.title),
            description: null,
            priority: 'normal',
            due_date: null,
            completed: false,
            position: 0,
            total_seconds: 0,
            tags: [],
            created_at: now(),
            updated_at: now(),
          }
          db.tasks[parentTask.list_id] = [...(db.tasks[parentTask.list_id] ?? []), subtask]
          return subtask
        }

        case 'get_subtasks': {
          const parentId = Number(args.task_id)
          return Object.values(db.tasks).flat().filter(t => t.parent_task_id === parentId)
        }
```

Also update `create_task` to add `parent_task_id: null` to the task object.

- [ ] **Step 2: Run full Playwright suite**

```powershell
npx playwright test --reporter=list
```

Expected: All 6 tests pass.

- [ ] **Step 3: Run full Rust test suite**

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cd src-tauri && cargo test -- --nocapture
```

Expected: All tests pass.

- [ ] **Step 4: Run full Vitest suite**

```powershell
npm test
```

Expected: All tests pass.

- [ ] **Step 5: Final commit**

```powershell
git add e2e/list-deletion.spec.ts
git commit -m "test(e2e): update Playwright mock for create_subtask and get_subtasks"
```
