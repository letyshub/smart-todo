import { describe, it, expect, vi } from 'vitest'
import { act, renderHook } from '@testing-library/react'
import { useResizablePanel, clampWidth } from '../useResizablePanel'

function fakePointerEvent(clientX: number) {
  return {
    clientX,
    preventDefault: () => {},
    pointerId: 1,
    currentTarget: {
      setPointerCapture: () => {},
      releasePointerCapture: () => {},
      hasPointerCapture: () => true,
    },
  } as unknown as React.PointerEvent<HTMLDivElement>
}

describe('clampWidth', () => {
  it('clamps below the minimum', () => {
    expect(clampWidth(100, 200, 400)).toBe(200)
  })

  it('clamps above the maximum', () => {
    expect(clampWidth(500, 200, 400)).toBe(400)
  })

  it('passes values within range through unchanged', () => {
    expect(clampWidth(300, 200, 400)).toBe(300)
  })
})

describe('useResizablePanel', () => {
  it('grows a left-side panel when dragging the handle right', () => {
    const onCommit = vi.fn()
    const { result } = renderHook(() =>
      useResizablePanel({ width: 240, min: 200, max: 420, side: 'left', onCommit })
    )

    act(() => result.current.handleProps.onPointerDown(fakePointerEvent(100)))
    act(() => result.current.handleProps.onPointerMove(fakePointerEvent(150)))
    expect(result.current.width).toBe(290)
    expect(result.current.isDragging).toBe(true)

    act(() => result.current.handleProps.onPointerUp(fakePointerEvent(150)))
    expect(result.current.isDragging).toBe(false)
    expect(onCommit).toHaveBeenCalledWith(290)
  })

  it('grows a right-side panel when dragging the handle left', () => {
    const onCommit = vi.fn()
    const { result } = renderHook(() =>
      useResizablePanel({ width: 320, min: 280, max: 640, side: 'right', onCommit })
    )

    act(() => result.current.handleProps.onPointerDown(fakePointerEvent(300)))
    act(() => result.current.handleProps.onPointerMove(fakePointerEvent(250)))
    expect(result.current.width).toBe(370)

    act(() => result.current.handleProps.onPointerUp(fakePointerEvent(250)))
    expect(onCommit).toHaveBeenCalledWith(370)
  })

  it('clamps the live width during drag to the min/max bounds', () => {
    const onCommit = vi.fn()
    const { result } = renderHook(() =>
      useResizablePanel({ width: 240, min: 200, max: 420, side: 'left', onCommit })
    )

    act(() => result.current.handleProps.onPointerDown(fakePointerEvent(100)))
    act(() => result.current.handleProps.onPointerMove(fakePointerEvent(-1000)))
    expect(result.current.width).toBe(200)

    act(() => result.current.handleProps.onPointerUp(fakePointerEvent(-1000)))
    expect(onCommit).toHaveBeenCalledWith(200)
  })
})
