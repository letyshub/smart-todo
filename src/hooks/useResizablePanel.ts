import { useCallback, useRef, useState } from 'react'

interface UseResizablePanelOptions {
  /** Committed width in px (from settings / parent state) */
  width: number
  min: number
  max: number
  /** Which side of the screen the panel sits on — determines drag direction */
  side: 'left' | 'right'
  /** Called once, on drag release, with the clamped final width */
  onCommit: (width: number) => void
}

export function clampWidth(width: number, min: number, max: number) {
  return Math.min(max, Math.max(min, width))
}

export function useResizablePanel({ width, min, max, side, onCommit }: UseResizablePanelOptions) {
  const [liveWidth, setLiveWidth] = useState(width)
  const [isDragging, setIsDragging] = useState(false)
  const startX = useRef(0)
  const startWidth = useRef(0)

  const handlePointerDown = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault()
    startX.current = e.clientX
    startWidth.current = width
    setLiveWidth(width)
    setIsDragging(true)
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
    e.currentTarget.setPointerCapture(e.pointerId)
  }, [width])

  const handlePointerMove = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDragging) return
    const delta = e.clientX - startX.current
    const signedDelta = side === 'left' ? delta : -delta
    setLiveWidth(clampWidth(startWidth.current + signedDelta, min, max))
  }, [isDragging, side, min, max])

  const endDrag = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDragging) return
    setIsDragging(false)
    document.body.style.userSelect = ''
    document.body.style.cursor = ''
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId)
    }
    onCommit(liveWidth)
  }, [isDragging, liveWidth, onCommit])

  return {
    width: isDragging ? liveWidth : width,
    isDragging,
    handleProps: {
      onPointerDown: handlePointerDown,
      onPointerMove: handlePointerMove,
      onPointerUp: endDrag,
      onPointerCancel: endDrag,
    },
  }
}
