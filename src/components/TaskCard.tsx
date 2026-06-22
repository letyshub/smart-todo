import type { Task } from '../types'
import { isOverdue, isDueToday, formatDueDate } from '../lib/dateUtils'
import { formatTotal } from '../lib/timeUtils'
import TimerWidget from './TimerWidget'

interface Props {
  task: Task
  onOpen: (task: Task) => void
  onToggleComplete: (task: Task) => void
}

export default function TaskCard({ task, onOpen, onToggleComplete }: Props) {
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
        onChange={(e) => { e.stopPropagation(); onToggleComplete(task) }}
        className="w-4 h-4 rounded accent-indigo-600 cursor-pointer shrink-0"
      />

      {/* Main content */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          {task.priority === 'high' && (
            <span className="text-red-500 text-xs shrink-0" title="High priority">⚑</span>
          )}
          <span className={`text-sm font-medium truncate ${task.completed ? 'line-through text-gray-400 dark:text-gray-500' : 'text-gray-900 dark:text-gray-100'}`}>
            {task.title}
          </span>
        </div>

        {/* Tags */}
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

      {/* Right side: due date, total time, timer */}
      <div className="flex items-center gap-2 shrink-0">
        {task.due_date && (
          <span className={`text-xs ${overdue || dueToday ? 'text-red-500 font-medium' : 'text-gray-400 dark:text-gray-500'}`}>
            {formatDueDate(task.due_date)}
          </span>
        )}
        {task.total_seconds > 0 && (
          <span className="text-xs text-gray-400 dark:text-gray-500">
            {formatTotal(task.total_seconds)}
          </span>
        )}
        {!task.completed && <TimerWidget taskId={task.id} />}
      </div>
    </div>
  )
}
