import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import TaskEditor from '../TaskEditor'
import type { Task } from '../../types'

const baseTask: Task = {
  id: 1,
  list_id: 1,
  title: 'Parent task',
  description: null,
  priority: 'normal',
  due_date: null,
  completed: false,
  completed_at: null,
  position: 0,
  created_at: '2026-06-20T00:00:00',
  updated_at: '2026-06-20T00:00:00',
  parent_task_id: null,
  is_subtask: false,
  status: 'todo',
  tags: [],
  total_seconds: 0,
  subtask_total: 0,
  subtask_completed: 0,
}

const mockGetSubtasks = vi.fn()
const mockCreateSubtask = vi.fn()
const mockUpdateTask = vi.fn()
const mockDeleteTask = vi.fn()

vi.mock('../../lib/tauri', () => ({
  api: {
    getTimerSessions: vi.fn().mockResolvedValue([]),
    getAllTags: vi.fn().mockResolvedValue([]),
    getSubtasks: (...args: unknown[]) => mockGetSubtasks(...args),
    getChildTasks: vi.fn().mockResolvedValue([]),
    createSubtask: (...args: unknown[]) => mockCreateSubtask(...args),
    updateTask: (...args: unknown[]) => mockUpdateTask(...args),
    deleteTask: (...args: unknown[]) => mockDeleteTask(...args),
  },
}))

vi.mock('../../store/tasksStore', () => ({
  useTasksStore: Object.assign(
    () => ({
      update: vi.fn(),
      remove: vi.fn(),
      setTags: vi.fn(),
      loadList: vi.fn(),
      loadDashboard: vi.fn(),
    }),
    { getState: () => ({ dashboard: null }) }
  ),
}))

vi.mock('../../store/timerStore', () => ({
  useTimerStore: () => ({
    activeTaskId: null,
    elapsedSeconds: 0,
    start: vi.fn(),
    stop: vi.fn(),
  }),
}))

vi.mock('../../store/settingsStore', () => ({
  useSettingsStore: (selector: (s: { settings: null; setTaskEditorWidth: () => void }) => unknown) =>
    selector({ settings: null, setTaskEditorWidth: vi.fn() }),
}))

describe('TaskEditor subtask progress bar', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('hides subtask progress bar when there are no subtasks', async () => {
    mockGetSubtasks.mockResolvedValue([])
    render(<TaskEditor task={baseTask} listId={1} onClose={vi.fn()} />)
    await waitFor(() => expect(mockGetSubtasks).toHaveBeenCalledWith(1))
    expect(screen.queryByRole('progressbar')).toBeNull()
  })

  it('shows subtask progress bar and percentage when subtasks exist and updates on toggle', async () => {
    const sub1: Task = { ...baseTask, id: 10, parent_task_id: 1, is_subtask: true, title: 'Sub 1', completed: true }
    const sub2: Task = { ...baseTask, id: 11, parent_task_id: 1, is_subtask: true, title: 'Sub 2', completed: false }
    mockGetSubtasks.mockResolvedValue([sub1, sub2])

    render(<TaskEditor task={baseTask} listId={1} onClose={vi.fn()} />)

    await waitFor(() => expect(screen.getByText('Sub 1')).toBeTruthy())
    const bar = screen.getByRole('progressbar')
    expect(bar.getAttribute('aria-valuenow')).toBe('50')
    expect(screen.getByText('50%')).toBeTruthy()

    mockUpdateTask.mockResolvedValue({ ...sub2, completed: true })
    const checkboxes = screen.getAllByRole('checkbox')
    // Sub 2 checkbox is the last one (after High priority and Sub 1)
    fireEvent.click(checkboxes[checkboxes.length - 1])

    await waitFor(() => {
      expect(screen.getByRole('progressbar').getAttribute('aria-valuenow')).toBe('100')
      expect(screen.getByText('100%')).toBeTruthy()
    })
  })
})
