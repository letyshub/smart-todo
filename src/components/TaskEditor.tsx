import { useState, useEffect } from 'react'
import type { Task, Tag, TimerSession } from '../types'
import { useTasksStore } from '../store/tasksStore'
import { useTimerStore } from '../store/timerStore'
import { api } from '../lib/tauri'
import { formatTotal, formatLive } from '../lib/timeUtils'
import MarkdownRenderer from './MarkdownRenderer'
import TagInput from './TagInput'
import TaskCard from './TaskCard'

interface Props {
  task: Task
  listId: number
  onClose: () => void
  parentTask?: Task
  onBack?: () => void
  onOpenChildTask?: (task: Task) => void
}

export default function TaskEditor({ task, listId, onClose, parentTask, onBack, onOpenChildTask }: Props) {
  const { update, remove, setTags } = useTasksStore()
  const { activeTaskId, elapsedSeconds, start, stop } = useTimerStore()

  const [localTask, setLocalTask] = useState<Task>(task)
  const isTimerActive = activeTaskId === localTask.id

  const [title, setTitle] = useState(task.title)
  const [description, setDescription] = useState(task.description ?? '')
  const [preview, setPreview] = useState(false)
  const [sessions, setSessions] = useState<TimerSession[]>([])
  const [confirmDelete, setConfirmDelete] = useState(false)

  const [subtasks, setSubtasks] = useState<Task[]>([])
  const [newSubtask, setNewSubtask] = useState('')

  const [childTasks, setChildTasks] = useState<Task[]>([])
  const [newChildTitle, setNewChildTitle] = useState('')

  useEffect(() => {
    setLocalTask(task)
    setTitle(task.title)
    setDescription(task.description ?? '')
    setPreview(false)
    setConfirmDelete(false)
    api.getTimerSessions(task.id).then(setSessions).catch(console.error)
    api.getSubtasks(task.id).then(setSubtasks).catch(console.error)
    api.getChildTasks(task.id).then(setChildTasks).catch(console.error)
  }, [task.id])

  // Sync external updates (e.g. toggle complete from list view) for root tasks
  useEffect(() => {
    setLocalTask(task)
  }, [task.updated_at])

  async function callUpdate(fields: Parameters<typeof api.updateTask>[1]) {
    if (localTask.parent_task_id) {
      const updated = await api.updateTask(localTask.id, fields)
      setLocalTask(updated)
    } else {
      const updated = await update(localTask.id, listId, fields)
      setLocalTask(updated)
    }
  }

  async function handleTitleBlur() {
    if (title.trim() && title !== localTask.title) {
      await callUpdate({ title: title.trim() })
    }
  }

  async function handleDescriptionBlur() {
    if (description !== (localTask.description ?? '')) {
      await callUpdate({ description })
    }
  }

  async function handlePriorityToggle() {
    await callUpdate({ priority: localTask.priority === 'high' ? 'normal' : 'high' })
  }

  async function handleDueDateChange(e: React.ChangeEvent<HTMLInputElement>) {
    await callUpdate({ dueDate: e.target.value })
  }

  async function handleTagsChange(tagNames: string[]) {
    try {
      let tags: Tag[]
      if (localTask.parent_task_id) {
        tags = await api.setTaskTags(localTask.id, tagNames)
      } else {
        tags = await setTags(localTask.id, listId, tagNames)
      }
      setLocalTask((prev) => ({ ...prev, tags }))
    } catch (e) {
      console.error('set_task_tags failed:', e)
    }
  }

  async function handleTimerToggle() {
    if (isTimerActive) {
      await stop(localTask.id)
      const updated = await api.getTimerSessions(localTask.id)
      setSessions(updated)
    } else {
      await start(localTask.id)
    }
  }

  async function handleDelete() {
    await remove(localTask.id, listId)
    onClose()
  }

  async function handleAddSubtask() {
    const t = newSubtask.trim()
    if (!t) return
    const subtask = await api.createSubtask(localTask.id, t)
    setSubtasks((prev) => [...prev, subtask])
    setNewSubtask('')
  }

  async function handleToggleSubtask(subtask: Task) {
    const updated = await api.updateTask(subtask.id, { completed: !subtask.completed })
    setSubtasks((prev) => prev.map((s) => (s.id === updated.id ? updated : s)))
  }

  async function handleDeleteSubtask(subtask: Task) {
    await api.deleteTask(subtask.id)
    setSubtasks((prev) => prev.filter((s) => s.id !== subtask.id))
  }

  async function handleAddChildTask() {
    const t = newChildTitle.trim()
    if (!t) return
    const child = await api.createChildTask(localTask.id, t)
    setChildTasks((prev) => [...prev, child])
    setNewChildTitle('')
  }

  async function handleToggleChildTask(child: Task) {
    const updated = await api.updateTask(child.id, { completed: !child.completed })
    setChildTasks((prev) => prev.map((c) => (c.id === updated.id ? updated : c)))
  }

  async function handleDeleteChildTask(child: Task) {
    if (!window.confirm(`Delete "${child.title}"?`)) return
    await api.deleteTask(child.id)
    setChildTasks((prev) => prev.filter((c) => c.id !== child.id))
  }

  const totalSeconds = localTask.total_seconds + (isTimerActive ? elapsedSeconds : 0)

  return (
    <div className="w-80 shrink-0 border-l border-gray-200 dark:border-gray-700 flex flex-col h-full overflow-hidden bg-white dark:bg-gray-900">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-gray-100 dark:border-gray-800">
        <div className="flex items-center gap-2 min-w-0">
          {parentTask && onBack ? (
            <button
              onClick={onBack}
              className="flex items-center gap-1 text-xs text-indigo-500 hover:text-indigo-700 dark:hover:text-indigo-300 truncate max-w-[180px]"
              title={`Back to ${parentTask.title}`}
            >
              ← <span className="truncate">{parentTask.title}</span>
            </button>
          ) : (
            <span className="text-xs text-gray-400 uppercase tracking-wide">Task</span>
          )}
        </div>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 text-xl leading-none shrink-0"
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
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={localTask.priority === 'high'}
            onChange={handlePriorityToggle}
            className="w-4 h-4 accent-red-500 cursor-pointer"
          />
          <span className={`text-sm ${localTask.priority === 'high' ? 'text-red-600 dark:text-red-400 font-medium' : 'text-gray-600 dark:text-gray-400'}`}>
            High priority
          </span>
        </label>

        {/* Status */}
        <div className="flex items-center gap-1">
          {(['todo', 'inprogress', 'done'] as const).map((s) => (
            <button
              key={s}
              onClick={() => callUpdate({ status: s })}
              className={`flex-1 text-xs py-1 rounded transition-colors ${
                localTask.status === s
                  ? s === 'todo'
                    ? 'bg-gray-200 dark:bg-gray-700 text-gray-800 dark:text-gray-200 font-medium'
                    : s === 'inprogress'
                    ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300 font-medium'
                    : 'bg-green-100 dark:bg-green-900/50 text-green-700 dark:text-green-300 font-medium'
                  : 'text-gray-400 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800'
              }`}
            >
              {s === 'todo' ? 'Todo' : s === 'inprogress' ? 'In Progress' : 'Done'}
            </button>
          ))}
        </div>

        {/* Due Date */}
        <div className="flex items-center gap-2">
          <span className="text-xs text-gray-500 w-20">Due date</span>
          <input
            type="date"
            value={localTask.due_date ?? ''}
            onChange={handleDueDateChange}
            className="text-xs border border-gray-200 dark:border-gray-700 rounded px-2 py-1 bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 outline-none"
          />
          {localTask.due_date && (
            <button
              onClick={() => callUpdate({ dueDate: '' })}
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
          <TagInput tags={localTask.tags} onChange={handleTagsChange} />
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

        {/* Subtasks (lightweight checkboxes) */}
        <div>
          <span className="text-xs text-gray-500 block mb-2">
            Subtasks
            {subtasks.length > 0 && (
              <span className="ml-1 text-gray-400">
                ({subtasks.filter((s) => s.completed).length}/{subtasks.length})
              </span>
            )}
          </span>
          {subtasks.map((s) => (
            <div key={s.id} className="flex items-center gap-2 py-1 group/subtask">
              <input
                type="checkbox"
                checked={s.completed}
                onChange={() => handleToggleSubtask(s)}
                className="w-3.5 h-3.5 accent-indigo-600 cursor-pointer shrink-0"
              />
              <span className={`text-sm flex-1 ${s.completed ? 'line-through text-gray-400 dark:text-gray-500' : 'text-gray-700 dark:text-gray-300'}`}>
                {s.title}
              </span>
              <button
                onClick={() => handleDeleteSubtask(s)}
                className="opacity-0 group-hover/subtask:opacity-100 text-gray-400 hover:text-red-500 transition-opacity text-base leading-none"
                aria-label={`Delete subtask ${s.title}`}
              >
                ×
              </button>
            </div>
          ))}
          <form
            onSubmit={(e) => { e.preventDefault(); handleAddSubtask() }}
            className="flex gap-1 mt-1"
          >
            <input
              className="flex-1 text-xs px-2 py-1 border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none"
              placeholder="Add subtask…"
              value={newSubtask}
              onChange={(e) => setNewSubtask(e.target.value)}
            />
            <button
              type="submit"
              className="text-xs px-2 py-1 bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300 rounded hover:bg-indigo-200 dark:hover:bg-indigo-800"
            >
              Add
            </button>
          </form>
        </div>

        {/* Child Tasks (full tasks, clickable) */}
        <div>
          <span className="text-xs text-gray-500 block mb-2">Child Tasks</span>
          {childTasks.map((child) => (
            <TaskCard
              key={child.id}
              task={child}
              onOpen={(t) => onOpenChildTask?.(t)}
              onToggleComplete={handleToggleChildTask}
              onDelete={handleDeleteChildTask}
            />
          ))}
          <form
            onSubmit={(e) => { e.preventDefault(); handleAddChildTask() }}
            className="flex gap-1 mt-1"
          >
            <input
              className="flex-1 text-xs px-2 py-1 border border-gray-200 dark:border-gray-700 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none"
              placeholder="Add child task…"
              value={newChildTitle}
              onChange={(e) => setNewChildTitle(e.target.value)}
            />
            <button
              type="submit"
              className="text-xs px-2 py-1 bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300 rounded hover:bg-indigo-200 dark:hover:bg-indigo-800"
            >
              Add
            </button>
          </form>
        </div>

        {/* Timer */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs text-gray-500">Timer</span>
            <span className="text-xs text-gray-400">
              {totalSeconds > 0 ? `${formatTotal(totalSeconds)} total` : 'No time tracked'}
            </span>
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
