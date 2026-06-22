import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import MarkdownRenderer from '../MarkdownRenderer'

describe('MarkdownRenderer', () => {
  it('renders bold markdown inside a <strong> element', () => {
    const { container } = render(<MarkdownRenderer content="**Hello**" />)
    expect(container.querySelector('strong')).toBeTruthy()
    expect(screen.getByText('Hello')).toBeTruthy()
  })

  it('renders GFM strikethrough inside a <del> element', () => {
    const { container } = render(<MarkdownRenderer content="~~deleted~~" />)
    expect(container.querySelector('del')).toBeTruthy()
    expect(screen.getByText('deleted')).toBeTruthy()
  })

  it('strips javascript: href from links', () => {
    const { container } = render(
      <MarkdownRenderer content="[click me](javascript:alert(1))" />
    )
    const link = container.querySelector('a')
    expect(link).toBeTruthy()
    expect(link?.getAttribute('href')).toBeNull()
  })

  it('does not render raw script tags', () => {
    const { container } = render(
      <MarkdownRenderer content="<script>alert('xss')</script>normal text" />
    )
    expect(container.querySelector('script')).toBeNull()
    expect(screen.getByText(/normal text/)).toBeTruthy()
  })
})
