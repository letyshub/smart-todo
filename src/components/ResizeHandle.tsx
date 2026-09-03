interface Props {
  side: 'left' | 'right'
  isDragging: boolean
  onPointerDown: (e: React.PointerEvent<HTMLDivElement>) => void
  onPointerMove: (e: React.PointerEvent<HTMLDivElement>) => void
  onPointerUp: (e: React.PointerEvent<HTMLDivElement>) => void
  onPointerCancel: (e: React.PointerEvent<HTMLDivElement>) => void
}

export default function ResizeHandle({ side, isDragging, ...handlers }: Props) {
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      className={`absolute top-0 bottom-0 w-1.5 cursor-col-resize group/handle z-10 ${
        side === 'left' ? '-right-0.5' : '-left-0.5'
      }`}
      {...handlers}
    >
      <div
        className={`h-full w-px mx-auto transition-colors ${
          isDragging
            ? 'bg-indigo-500 w-0.5'
            : 'bg-transparent group-hover/handle:bg-indigo-400 dark:group-hover/handle:bg-indigo-500'
        }`}
      />
    </div>
  )
}
