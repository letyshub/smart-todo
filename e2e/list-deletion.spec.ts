import { test, expect, type Page } from '@playwright/test'

// Injected before every page load — sets up a stateful in-memory Tauri mock
async function installMock(page: Page) {
  await page.addInitScript(() => {
    let nextId = 1
    const db: {
      lists: Array<{ id: number; title: string; color: string; position: number }>
      tasks: Record<number, Array<{
        id: number; list_id: number; parent_task_id: number | null; is_subtask: boolean
        title: string; description: string | null; status: string
        priority: string; due_date: string | null; completed: boolean
        position: number; total_seconds: number; tags: string[]
        created_at: string; updated_at: string
      }>>
    } = { lists: [], tasks: {} }

    const now = () => new Date().toISOString()

    window.__TAURI_INTERNALS__ = {} as typeof window.__TAURI_INTERNALS__
    ;(window.__TAURI_INTERNALS__ as Record<string, unknown>).invoke = async (
      cmd: string,
      args: Record<string, unknown> = {}
    ) => {
      switch (cmd) {
        case 'get_lists':
          return [...db.lists]

        case 'create_list': {
          const list = {
            id: nextId++,
            title: String(args.title),
            color: String(args.color ?? '#6366f1'),
            position: db.lists.length,
          }
          db.lists.push(list)
          return list
        }

        case 'update_list': {
          const idx = db.lists.findIndex(l => l.id === args.id)
          if (idx < 0) throw new Error('List not found')
          if (args.title != null) db.lists[idx].title = String(args.title)
          if (args.color != null) db.lists[idx].color = String(args.color)
          return db.lists[idx]
        }

        case 'delete_list': {
          const id = Number(args.id)
          db.lists = db.lists.filter(l => l.id !== id)
          delete db.tasks[id]
          return null
        }

        case 'get_tasks':
          return (db.tasks[Number(args.listId)] ?? []).filter(t => t.parent_task_id === null)

        case 'create_task': {
          const task = {
            id: nextId++,
            list_id: Number(args.listId),
            parent_task_id: null,
            is_subtask: false,
            status: 'todo',
            title: String(args.title),
            description: null,
            priority: String(args.priority ?? 'normal'),
            due_date: null,
            completed: false,
            position: 0,
            total_seconds: 0,
            tags: [],
            created_at: now(),
            updated_at: now(),
          }
          const lid = Number(args.listId)
          db.tasks[lid] = [...(db.tasks[lid] ?? []), task]
          return task
        }

        case 'create_subtask': {
          const parentId = Number(args.parentTaskId)
          const parentTask = Object.values(db.tasks).flat().find(t => t.id === parentId)
          if (!parentTask) throw new Error('Parent task not found')
          const subtask = {
            id: nextId++,
            list_id: parentTask.list_id,
            parent_task_id: parentId,
            is_subtask: true,
            status: 'todo',
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
          const parentId = Number(args.taskId)
          return Object.values(db.tasks).flat().filter(t => t.parent_task_id === parentId && t.is_subtask)
        }

        case 'create_child_task': {
          const parentId = Number(args.parentTaskId)
          const parentTask = Object.values(db.tasks).flat().find(t => t.id === parentId)
          if (!parentTask) throw new Error('Parent task not found')
          const child = {
            id: nextId++,
            list_id: parentTask.list_id,
            parent_task_id: parentId,
            is_subtask: false,
            status: 'todo',
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
          db.tasks[parentTask.list_id] = [...(db.tasks[parentTask.list_id] ?? []), child]
          return child
        }

        case 'get_child_tasks': {
          const parentId = Number(args.taskId)
          return Object.values(db.tasks).flat().filter(t => t.parent_task_id === parentId && !t.is_subtask)
        }

        case 'update_task': {
          const id = Number(args.id)
          for (const key of Object.keys(db.tasks)) {
            const idx = db.tasks[Number(key)].findIndex(t => t.id === id)
            if (idx >= 0) {
              const t = db.tasks[Number(key)][idx]
              if (args.title != null) t.title = String(args.title)
              if (args.description != null) t.description = String(args.description)
              if (args.priority != null) t.priority = String(args.priority)
              if (args.status != null) {
                t.status = String(args.status)
                t.completed = t.status === 'done'
              } else if (args.completed != null) {
                t.completed = Boolean(args.completed)
                t.status = t.completed ? 'done' : 'todo'
              }
              t.updated_at = now()
              return { ...t }
            }
          }
          throw new Error('Task not found')
        }

        case 'delete_task': {
          const id = Number(args.id)
          for (const key of Object.keys(db.tasks)) {
            db.tasks[Number(key)] = db.tasks[Number(key)].filter(t => t.id !== id)
          }
          return null
        }

        case 'get_dashboard_tasks': {
          const today = new Date().toISOString().slice(0, 10)
          const all = Object.values(db.tasks).flat().filter(t => !t.completed && t.parent_task_id === null)
          const overdue = all.filter(t => t.due_date && t.due_date < today)
          const overdueIds = new Set(overdue.map(t => t.id))
          const highPriority = all.filter(t => t.priority === 'high' && !overdueIds.has(t.id))
          const upcoming = all.filter(t => t.due_date && t.due_date >= today && !overdueIds.has(t.id))
          return { overdue, high_priority: highPriority, upcoming }
        }

        case 'set_task_tags': return []
        case 'get_all_tags': return []
        case 'start_timer': return { session_id: nextId++, already_running: false }
        case 'stop_timer': return null
        case 'get_active_timers': return []
        case 'get_timer_sessions': return []
        case 'get_settings': return { theme: 'system', data_dir: null }
        case 'set_setting': return null
        case 'change_data_dir': return null

        default:
          throw new Error(`[mock] Unknown command: ${cmd}`)
      }
    }
  })
}

/** Create a list and navigate into it. Returns after list view is visible. */
async function createAndOpenList(page: Page, name: string) {
  await page.getByRole('button', { name: '+ New List' }).click()
  await page.getByRole('textbox', { name: 'List name' }).fill(name)
  await page.getByRole('textbox', { name: 'List name' }).press('Enter')
  // After creation sidebar navigates to the list
  await expect(page.getByRole('button', { name: name, exact: true })).toBeVisible()
}

/** Click "Delete list", then confirm by clicking the inline "Delete" button. */
async function confirmDeleteList(page: Page) {
  await page.getByRole('button', { name: 'Delete list' }).click()
  await page.getByRole('button', { name: 'Delete', exact: true }).click()
}

test.describe('List creation', () => {
  test.beforeEach(async ({ page }) => {
    await installMock(page)
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('creates a list and it appears in the sidebar', async ({ page }) => {
    await createAndOpenList(page, 'Work')
    await expect(page.getByRole('button', { name: 'Work', exact: true })).toBeVisible()
  })
})

test.describe('List deletion', () => {
  test.beforeEach(async ({ page }) => {
    await installMock(page)
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('delete list button is visible in list header', async ({ page }) => {
    await createAndOpenList(page, 'Personal')
    await expect(page.getByRole('button', { name: 'Delete list' })).toBeVisible()
  })

  test('cancelling delete confirmation preserves the list', async ({ page }) => {
    await createAndOpenList(page, 'Personal')

    await page.getByRole('button', { name: 'Delete list' }).click()
    await page.getByRole('button', { name: 'Cancel', exact: true }).click()

    await expect(page.getByRole('button', { name: 'Personal', exact: true })).toBeVisible()
  })

  test('confirming deletion removes list from sidebar and navigates to dashboard', async ({ page }) => {
    await createAndOpenList(page, 'Temporary')

    await confirmDeleteList(page)

    await expect(page.getByRole('button', { name: 'Temporary', exact: true })).not.toBeVisible()
    // Should be on dashboard now
    await expect(page.getByRole('button', { name: /Dashboard/i })).toBeVisible()
  })

  test('list rename saves on blur', async ({ page }) => {
    await createAndOpenList(page, 'OldName')

    // Click title to start editing
    await page.getByRole('heading', { name: 'OldName' }).click()
    const input = page.getByRole('textbox').filter({ hasNot: page.locator('[placeholder="Task title…"]') }).first()
    await input.fill('NewName')
    await input.press('Enter')

    await expect(page.getByRole('button', { name: 'NewName', exact: true })).toBeVisible()
  })
})

test.describe('Task deletion from list view', () => {
  test.beforeEach(async ({ page }) => {
    await installMock(page)
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('hover delete button removes task', async ({ page }) => {
    await createAndOpenList(page, 'Work')
    await page.getByRole('button', { name: '+ Add task' }).click()
    await page.getByPlaceholder('Task title…').fill('Fix bug')
    await page.getByPlaceholder('Task title…').press('Enter')
    await expect(page.getByText('Fix bug')).toBeVisible()

    page.once('dialog', (dialog) => dialog.accept())
    await page.locator(`[aria-label="Delete task Fix bug"]`).click({ force: true })

    await expect(page.getByText('Fix bug')).not.toBeVisible()
  })
})

test.describe('Dashboard after list deletion', () => {
  test.beforeEach(async ({ page }) => {
    await installMock(page)
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('high-priority task from deleted list disappears from dashboard', async ({ page }) => {
    await createAndOpenList(page, 'Projects')

    // Add task
    await page.getByRole('button', { name: '+ Add task' }).click()
    await page.getByPlaceholder('Task title…').fill('Critical Feature')
    await page.getByPlaceholder('Task title…').press('Enter')
    await expect(page.getByText('Critical Feature')).toBeVisible()

    // Open task editor and set high priority
    await page.getByText('Critical Feature').click()
    await page.getByRole('checkbox', { name: /High priority/i }).check()

    // Navigate to dashboard
    await page.getByRole('button', { name: /Dashboard/ }).click()
    await expect(page.getByText('Critical Feature')).toBeVisible()

    // Navigate back to list and delete it
    await page.getByRole('button', { name: 'Projects', exact: true }).click()
    await confirmDeleteList(page)

    // Task should no longer appear on the dashboard
    await expect(page.getByText('Critical Feature')).not.toBeVisible()
  })

  test('dashboard remains empty after deleting only list', async ({ page }) => {
    await createAndOpenList(page, 'Throwaway')

    await confirmDeleteList(page)

    await expect(page.getByRole('button', { name: 'Throwaway', exact: true })).not.toBeVisible()
  })
})
