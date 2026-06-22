import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import TaskCard from '../TaskCard'
import type { Task } from '../../types'

const baseTask: Task = {
  id: 1,
  list_id: 1,
  title: 'Test task',
  description: null,
  priority: 'normal',
  due_date: null,
  completed: false,
  completed_at: null,
  position: 0,
  created_at: '2026-06-20T00:00:00',
  updated_at: '2026-06-20T00:00:00',
  tags: [],
  total_seconds: 0,
}

// Mock the timer store so TimerWidget doesn't break in jsdom
vi.mock('../../store/timerStore', () => ({
  useTimerStore: () => ({ activeTaskId: null, elapsedSeconds: 0, start: vi.fn(), stop: vi.fn() }),
}))

describe('TaskCard', () => {
  it('renders the task title', () => {
    render(<TaskCard task={baseTask} onOpen={vi.fn()} onToggleComplete={vi.fn()} />)
    expect(screen.getByText('Test task')).toBeTruthy()
  })

  it('shows high-priority indicator for high-priority tasks', () => {
    const task = { ...baseTask, priority: 'high' as const }
    const { container } = render(<TaskCard task={task} onOpen={vi.fn()} onToggleComplete={vi.fn()} />)
    expect(container.textContent).toContain('⚑')
  })

  it('calls onOpen when the card is clicked', () => {
    const onOpen = vi.fn()
    render(<TaskCard task={baseTask} onOpen={onOpen} onToggleComplete={vi.fn()} />)
    fireEvent.click(screen.getByText('Test task'))
    expect(onOpen).toHaveBeenCalledWith(baseTask)
  })

  it('calls onToggleComplete when checkbox changes', () => {
    const onToggleComplete = vi.fn()
    render(<TaskCard task={baseTask} onOpen={vi.fn()} onToggleComplete={onToggleComplete} />)
    const checkbox = screen.getByRole('checkbox')
    fireEvent.click(checkbox)
    expect(onToggleComplete).toHaveBeenCalledWith(baseTask)
  })

  it('renders tag chips', () => {
    const task = { ...baseTask, tags: [{ id: 1, name: 'work', color: null }] }
    render(<TaskCard task={task} onOpen={vi.fn()} onToggleComplete={vi.fn()} />)
    expect(screen.getByText('work')).toBeTruthy()
  })

  it('applies strikethrough to completed task title', () => {
    const task = { ...baseTask, completed: true }
    const { container } = render(<TaskCard task={task} onOpen={vi.fn()} onToggleComplete={vi.fn()} />)
    const titleSpan = container.querySelector('.line-through')
    expect(titleSpan).toBeTruthy()
  })

  it('shows due date badge', () => {
    const task = { ...baseTask, due_date: '2030-12-31' }
    render(<TaskCard task={task} onOpen={vi.fn()} onToggleComplete={vi.fn()} />)
    expect(screen.getByText(/Dec/)).toBeTruthy()
  })
})
