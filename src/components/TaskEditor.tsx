import { useState, useEffect } from 'react'
import type { Task, TimerSession } from '../types'
import { useTasksStore } from '../store/tasksStore'
import { useTimerStore } from '../store/timerStore'
import { api } from '../lib/tauri'
import { formatTotal, formatLive } from '../lib/timeUtils'
import MarkdownRenderer from './MarkdownRenderer'
import TagInput from './TagInput'

interface Props {
  task: Task
  listId: number
  onClose: () => void
}

export default function TaskEditor({ task, listId, onClose }: Props) {
  const { update, remove, setTags } = useTasksStore()
  const { activeTaskId, elapsedSeconds, start, stop } = useTimerStore()
  const isTimerActive = activeTaskId === task.id

  const [title, setTitle] = useState(task.title)
  const [description, setDescription] = useState(task.description ?? '')
  const [preview, setPreview] = useState(false)
  const [sessions, setSessions] = useState<TimerSession[]>([])
  const [confirmDelete, setConfirmDelete] = useState(false)

  // Sync local state when task prop changes (user clicks different task)
  useEffect(() => {
    setTitle(task.title)
    setDescription(task.description ?? '')
    setPreview(false)
    setConfirmDelete(false)
    api.getTimerSessions(task.id).then(setSessions)
  }, [task.id])

  async function handleTitleBlur() {
    if (title.trim() && title !== task.title) {
      await update(task.id, listId, { title: title.trim() })
    }
  }

  async function handleDescriptionBlur() {
    if (description !== (task.description ?? '')) {
      await update(task.id, listId, { description })
    }
  }

  async function handlePriorityToggle() {
    const next = task.priority === 'high' ? 'normal' : 'high'
    await update(task.id, listId, { priority: next })
  }

  async function handleDueDateChange(e: React.ChangeEvent<HTMLInputElement>) {
    await update(task.id, listId, { dueDate: e.target.value })
  }

  async function handleTagsChange(tagNames: string[]) {
    await setTags(task.id, listId, tagNames)
  }

  async function handleTimerToggle() {
    if (isTimerActive) {
      await stop(task.id)
      const updated = await api.getTimerSessions(task.id)
      setSessions(updated)
    } else {
      await start(task.id)
    }
  }

  async function handleDelete() {
    await remove(task.id, listId)
    onClose()
  }

  const totalSeconds = task.total_seconds + (isTimerActive ? elapsedSeconds : 0)

  return (
    <div className="w-80 shrink-0 border-l border-gray-200 dark:border-gray-700 flex flex-col h-full overflow-hidden bg-white dark:bg-gray-900">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-100 dark:border-gray-800">
        <span className="text-xs text-gray-400 uppercase tracking-wide">Task</span>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 text-xl leading-none"
          aria-label="Close"
        >
          ×
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
        {/* Title */}
        <input
          className="w-full text-base font-semibold bg-transparent border-b border-transparent hover:border-gray-200 dark:hover:border-gray-700 focus:border-indigo-400 dark:focus:border-indigo-500 outline-none text-gray-900 dark:text-gray-100 pb-1"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={handleTitleBlur}
        />

        {/* Priority */}
        <div className="flex items-center gap-2">
          <span className="text-xs text-gray-500 w-20">Priority</span>
          <button
            onClick={handlePriorityToggle}
            className={`text-xs px-2 py-1 rounded transition-colors ${
              task.priority === 'high'
                ? 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400'
            }`}
          >
            {task.priority === 'high' ? '⚑ High' : 'Normal'}
          </button>
        </div>

        {/* Due Date */}
        <div className="flex items-center gap-2">
          <span className="text-xs text-gray-500 w-20">Due date</span>
          <input
            type="date"
            value={task.due_date ?? ''}
            onChange={handleDueDateChange}
            className="text-xs border border-gray-200 dark:border-gray-700 rounded px-2 py-1 bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 outline-none"
          />
          {task.due_date && (
            <button
              onClick={() => update(task.id, listId, { dueDate: '' })}
              className="text-xs text-gray-400 hover:text-red-500"
              title="Clear due date"
            >
              ✕
            </button>
          )}
        </div>

        {/* Tags */}
        <div>
          <span className="text-xs text-gray-500 block mb-1">Tags</span>
          <TagInput tags={task.tags} onChange={handleTagsChange} />
        </div>

        {/* Description */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs text-gray-500">Description</span>
            <button
              onClick={() => setPreview((v) => !v)}
              className="text-xs text-indigo-500 hover:text-indigo-700"
            >
              {preview ? 'Edit' : 'Preview'}
            </button>
          </div>
          {preview ? (
            <div className="min-h-[80px] text-sm">
              {description ? (
                <MarkdownRenderer content={description} />
              ) : (
                <span className="text-gray-400 text-xs">No description</span>
              )}
            </div>
          ) : (
            <textarea
              className="w-full text-sm px-3 py-2 border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none resize-none min-h-[80px]"
              placeholder="Add a description (Markdown supported)…"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              onBlur={handleDescriptionBlur}
              rows={4}
            />
          )}
        </div>

        {/* Timer */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs text-gray-500">Timer</span>
            {totalSeconds > 0 && (
              <span className="text-xs text-gray-400">{formatTotal(totalSeconds)} total</span>
            )}
          </div>
          <button
            onClick={handleTimerToggle}
            className={`flex items-center gap-2 text-sm px-3 py-2 rounded transition-colors w-full justify-center ${
              isTimerActive
                ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400'
                : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
            }`}
          >
            {isTimerActive ? (
              <>⏹ Stop — {formatLive(elapsedSeconds)}</>
            ) : (
              <>&#9654; Start timer</>
            )}
          </button>

          {/* Session history */}
          {sessions.length > 0 && (
            <div className="mt-2 space-y-1">
              {sessions.map((s) => (
                <div key={s.id} className="flex justify-between text-xs text-gray-400">
                  <span>{s.started_at.slice(0, 10)}</span>
                  <span>{s.duration_seconds ? formatTotal(s.duration_seconds) : '—'}</span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Delete */}
        <div className="pt-4 border-t border-gray-100 dark:border-gray-800">
          {confirmDelete ? (
            <div className="flex gap-2">
              <button
                onClick={handleDelete}
                className="flex-1 text-xs px-3 py-2 bg-red-500 text-white rounded hover:bg-red-600"
              >
                Confirm delete
              </button>
              <button
                onClick={() => setConfirmDelete(false)}
                className="flex-1 text-xs px-3 py-2 bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 rounded hover:bg-gray-200"
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              onClick={() => setConfirmDelete(true)}
              className="w-full text-xs px-3 py-2 text-red-500 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-900/20 rounded transition-colors"
            >
              Delete task
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
