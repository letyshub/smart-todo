import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import SyncConflicts from '../SyncConflicts'
import { readValue } from '../../store/syncStore'
import type { SyncConflict } from '../../types'

const resolve = vi.fn()
let conflicts: SyncConflict[] = []

vi.mock('../../store/syncStore', async () => {
  const actual = await vi.importActual<typeof import('../../store/syncStore')>(
    '../../store/syncStore'
  )
  return {
    ...actual,
    useSyncStore: (selector: (s: unknown) => unknown) => selector({ conflicts, resolve }),
  }
})

const conflict: SyncConflict = {
  id: 7,
  entity: 'task',
  uuid: 'u1',
  field: 'title',
  subject: 'Buy milk',
  kept: '"Buy oat milk"',
  discarded: '"Buy whole milk"',
  detected_at: '2026-09-04T10:00:00Z',
}

describe('SyncConflicts', () => {
  beforeEach(() => {
    conflicts = []
    resolve.mockClear()
  })

  it('stays out of the way when nothing diverged', () => {
    const { container } = render(<SyncConflicts />)
    expect(container).toBeEmptyDOMElement()
  })

  it('says how many edits were overridden', () => {
    conflicts = [conflict]
    render(<SyncConflicts />)
    expect(screen.getByText(/1 edit was overridden/)).toBeInTheDocument()
  })

  it('shows both values only once the user asks to review', () => {
    conflicts = [conflict]
    render(<SyncConflicts />)
    expect(screen.queryByText('Buy whole milk')).not.toBeInTheDocument()

    fireEvent.click(screen.getByText(/1 edit was overridden/))

    expect(screen.getByText('Buy oat milk')).toBeInTheDocument()
    expect(screen.getByText('Buy whole milk')).toBeInTheDocument()
  })

  it('restores the overridden value on request', () => {
    conflicts = [conflict]
    render(<SyncConflicts />)
    fireEvent.click(screen.getByText(/1 edit was overridden/))

    fireEvent.click(screen.getByText('Restore mine'))

    expect(resolve).toHaveBeenCalledWith(7, true)
  })

  it('dismisses without changing anything when the incoming value is fine', () => {
    conflicts = [conflict]
    render(<SyncConflicts />)
    fireEvent.click(screen.getByText(/1 edit was overridden/))

    fireEvent.click(screen.getByText('Keep this'))

    expect(resolve).toHaveBeenCalledWith(7, false)
  })
})

describe('readValue', () => {
  it('unwraps JSON so the user sees the value, not its encoding', () => {
    expect(readValue('"Buy milk"')).toBe('Buy milk')
    expect(readValue('42')).toBe('42')
  })

  it('labels an absent value rather than printing null', () => {
    expect(readValue('null')).toBe('(empty)')
    expect(readValue('""')).toBe('(empty)')
  })
})
