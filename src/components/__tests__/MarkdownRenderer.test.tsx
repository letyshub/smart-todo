import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import MarkdownRenderer from '../MarkdownRenderer'

describe('MarkdownRenderer', () => {
  it('renders markdown text', () => {
    render(<MarkdownRenderer content="**Hello**" />)
    expect(screen.getByText('Hello')).toBeTruthy()
  })

  it('renders GFM strikethrough', () => {
    render(<MarkdownRenderer content="~~deleted~~" />)
    expect(screen.getByText('deleted')).toBeTruthy()
  })

  it('does not render script tags', () => {
    const { container } = render(
      <MarkdownRenderer content="<script>alert('xss')</script>normal text" />
    )
    expect(container.querySelector('script')).toBeNull()
    expect(screen.getByText(/normal text/)).toBeTruthy()
  })
})
