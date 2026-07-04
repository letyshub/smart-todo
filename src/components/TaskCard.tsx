import type { Task } from '../types'
import { isOverdue, isDueToday, formatDueDate } from '../lib/dateUtils'
import { formatTotal } from '../lib/timeUtils'
import TimerWidget from './TimerWidget'

interface Props {
  task: Task
  onOpen: (task: Task) => void
  onToggleComplete: (task: Task) => void
  onDelete?: (task: Task) => void
}

export default function TaskCard({ task, onOpen, onToggleComplete, onDelete }: Props) {
  const overdue = !task.completed && isOverdue(task.due_date)
  const dueToday = !task.completed && isDueToday(task.due_date)

  return (
    <div
      className="flex items-center gap-3 px-4 py-3 border-b border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800/50 cursor-pointer group"
      onClick={() => onOpen(task)}
    >
      {/* Checkbox */}
      <input
        type="checkbox"
        checked={task.completed}
        onClick={(e) => e.stopPropagation()}
        onChange={(e) => { e.stopPropagation(); onToggleComplete(task) }}
        className="w-4 h-4 rounded accent-indigo-600 cursor-pointer shrink-0"
      />

      {/* Main content */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          {task.priority === 'high' && (
            <span className="text-red-500 text-xs shrink-0" title="High priority">⚑</span>
          )}
          {task.status === 'inprogress' && (
            <span className="text-xs px-1.5 py-0.5 rounded bg-blue-100 dark:bg-blue-900/40 text-blue-600 dark:text-blue-400 shrink-0">In Progress</span>
          )}
          <span className={`text-sm font-medium truncate ${task.completed ? 'line-through text-gray-400 dark:text-gray-500' : 'text-gray-900 dark:text-gray-100'}`}>
            {task.title}
          </span>
        </div>

        {task.tags.length > 0 && (
          <div className="flex flex-wrap gap-1 mt-1">
            {task.tags.map((tag) => (
              <span
                key={tag.id}
                className="text-xs px-1.5 py-0.5 rounded bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300"
              >
                {tag.name}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Right side */}
      <div className="flex items-center gap-2 shrink-0">
        {task.due_date && (
          <span className={`text-xs px-1.5 py-0.5 rounded ${
            overdue || dueToday
              ? 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400 font-medium'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400'
          }`}>
            {formatDueDate(task.due_date)}
          </span>
        )}
        {task.total_seconds > 0 && (
          <span className="text-xs text-gray-400 dark:text-gray-500">
            {formatTotal(task.total_seconds)}
          </span>
        )}
        {!task.completed && <TimerWidget taskId={task.id} />}
        {onDelete && (
          <button
            onClick={(e) => { e.stopPropagation(); onDelete(task) }}
            className="opacity-0 group-hover:opacity-100 text-gray-400 hover:text-red-500 transition-opacity text-base leading-none w-5 h-5 flex items-center justify-center rounded hover:bg-red-50 dark:hover:bg-red-900/20"
            aria-label={`Delete task ${task.title}`}
          >
            ×
          </button>
        )}
      </div>
    </div>
  )
}
