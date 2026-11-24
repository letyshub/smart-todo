import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import Sidebar from '../Sidebar'
import type { List } from '../../types'

const mockLists: List[] = [
  {
    id: 1,
    title: 'Empty List',
    color: '#6366f1',
    position: 0,
    created_at: '2026-06-20T00:00:00',
    task_total: 0,
    task_completed: 0,
  },
  {
    id: 2,
    title: 'Work',
    color: '#3b82f6',
    position: 1,
    created_at: '2026-06-20T00:00:00',
    task_total: 5,
    task_completed: 3,
  },
]

let mockTasksByList: Record<number, { completed: boolean }[]> = {}

vi.mock('../../store/listsStore', () => ({
  useListsStore: () => ({
    lists: mockLists,
    create: vi.fn(),
  }),
}))

vi.mock('../../store/tasksStore', () => ({
  useTasksStore: (selector?: (s: { tasks: Record<number, { completed: boolean }[]> }) => unknown) =>
    selector ? selector({ tasks: mockTasksByList }) : { tasks: mockTasksByList },
}))

vi.mock('../../store/settingsStore', () => ({
  useSettingsStore: (selector: (s: { settings: null; setSidebarWidth: () => void }) => unknown) =>
    selector({ settings: null, setSidebarWidth: vi.fn() }),
}))

describe('Sidebar list progress bar', () => {
  it('only renders progress bar for lists that have tasks', () => {
    mockTasksByList = {}
    render(<Sidebar view={{ type: 'dashboard' }} onNavigate={vi.fn()} />)

    const bars = screen.getAllByRole('progressbar')
    expect(bars).toHaveLength(1)
    expect(bars[0].getAttribute('aria-valuenow')).toBe('60')
    expect(screen.getByText('60%')).toBeTruthy()
  })

  it('reflects live tasksStore completion counts when list tasks are loaded', () => {
    mockTasksByList = {
      2: [{ completed: true }, { completed: true }, { completed: true }, { completed: false }],
    }
    render(<Sidebar view={{ type: 'list', id: 2 }} onNavigate={vi.fn()} />)

    const bar = screen.getByRole('progressbar')
    expect(bar.getAttribute('aria-valuenow')).toBe('75')
    expect(screen.getByText('75%')).toBeTruthy()
  })
})
