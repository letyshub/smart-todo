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
