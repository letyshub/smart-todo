import { describe, it, expect } from 'vitest'
import { formatTotal, formatLive } from '../timeUtils'

describe('formatTotal', () => {
  it('returns empty string for 0 seconds', () => {
    expect(formatTotal(0)).toBe('')
  })
  it('formats minutes only', () => {
    expect(formatTotal(90)).toBe('1m')
  })
  it('formats hours and minutes', () => {
    expect(formatTotal(3661)).toBe('1h 1m')
  })
})

describe('formatLive', () => {
  it('pads minutes and seconds', () => {
    expect(formatLive(65)).toBe('01:05')
  })
  it('formats zero as 00:00', () => {
    expect(formatLive(0)).toBe('00:00')
  })
})
