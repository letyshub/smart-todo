import type { Task } from '../types'

interface Props {
  task: Task
  listId: number
  onClose: () => void
}

export default function TaskEditor({ task, onClose }: Props) {
  return (
    <div className="w-80 border-l border-gray-200 dark:border-gray-700 p-4">
      <div className="flex justify-between items-center mb-4">
        <h3 className="font-semibold text-gray-900 dark:text-gray-100 truncate">{task.title}</h3>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 text-xl leading-none"
          aria-label="Close"
        >
          ×
        </button>
      </div>
      <p className="text-sm text-gray-400">Task editor coming soon…</p>
    </div>
  )
}
