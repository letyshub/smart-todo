import { useTimerStore } from '../store/timerStore'
import { formatLive } from '../lib/timeUtils'

interface Props {
  taskId: number
}

export default function TimerWidget({ taskId }: Props) {
  const { activeTaskId, elapsedSeconds, start, stop } = useTimerStore()
  const isActive = activeTaskId === taskId

  async function toggle() {
    if (isActive) {
      await stop(taskId)
    } else {
      await start(taskId)
    }
  }

  return (
    <button
      onClick={(e) => { e.stopPropagation(); toggle() }}
      className={`flex items-center gap-1 text-xs px-2 py-1 rounded transition-colors ${
        isActive
          ? 'bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300'
          : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
      }`}
      title={isActive ? 'Stop timer' : 'Start timer'}
    >
      {isActive ? '⏹' : '▶'}
      {isActive && <span>{formatLive(elapsedSeconds)}</span>}
    </button>
  )
}
