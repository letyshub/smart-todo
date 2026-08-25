import { useEffect, useState, useRef } from 'react'
import type { Task } from '../types'
import type { View } from '../App'
import { useListsStore } from '../store/listsStore'
import { useTasksStore } from '../store/tasksStore'
import TaskCard from '../components/TaskCard'
import TaskEditor from '../components/TaskEditor'
import { listToMarkdown } from '../lib/exportUtils'

interface Props {
  listId: number
  onNavigate: (v: View) => void
}

export default function ListDetail({ listId, onNavigate }: Props) {
  const lists = useListsStore((s) => s.lists)
  const { update: updateList, remove: removeList } = useListsStore()
  const { tasks, loadList, create, update, remove } = useTasksStore()
  const list = lists.find((l) => l.id === listId)
  const listTasks = tasks[listId] ?? []

  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [previousTask, setPreviousTask] = useState<Task | null>(null)
  const [showCompleted, setShowCompleted] = useState(false)
  const [addingTitle, setAddingTitle] = useState('')
  const [isAdding, setIsAdding] = useState(false)

  const [editingTitle, setEditingTitle] = useState(false)
  const [titleDraft, setTitleDraft] = useState('')
  const titleInputRef = useRef<HTMLInputElement>(null)
  const [confirmDeleteList, setConfirmDeleteList] = useState(false)
  const [activeTagFilter, setActiveTagFilter] = useState<string | null>(null)
  const [copyFeedback, setCopyFeedback] = useState(false)

  useEffect(() => {
    loadList(listId)
  }, [listId, loadList])

  useEffect(() => {
    if (list) setTitleDraft(list.title)
  }, [list?.id])

  // Reset tag filter when switching lists
  useEffect(() => {
    setActiveTagFilter(null)
  }, [listId])

  // Collect unique tags present in this list
  const allTags = Array.from(
    new Map(
      listTasks.flatMap((t) => t.tags).map((tag) => [tag.name, tag])
    ).values()
  ).sort((a, b) => a.name.localeCompare(b.name))

  const filterByTag = (tasks: Task[]) =>
    activeTagFilter ? tasks.filter((t) => t.tags.some((tag) => tag.name === activeTagFilter)) : tasks

  const incomplete = filterByTag(listTasks.filter((t) => !t.completed))
  const completed = filterByTag(listTasks.filter((t) => t.completed))

  async function handleAdd() {
    const title = addingTitle.trim()
    if (!title) { setIsAdding(false); return }
    const task = await create(listId, title)
    setAddingTitle('')
    setIsAdding(false)
    setSelectedTask(task)
  }

  async function handleToggleComplete(task: Task) {
    await update(task.id, listId, { completed: !task.completed })
  }

  async function handleDeleteTask(task: Task) {
    if (!window.confirm(`Delete "${task.title}"?`)) return
    if (selectedTask?.id === task.id) setSelectedTask(null)
    await remove(task.id, listId)
  }

  function handleStartEditTitle() {
    setTitleDraft(list?.title ?? '')
    setEditingTitle(true)
    setTimeout(() => titleInputRef.current?.select(), 0)
  }

  async function handleSaveTitle() {
    setEditingTitle(false)
    const trimmed = titleDraft.trim()
    if (trimmed && trimmed !== list?.title) {
      await updateList(listId, trimmed)
    }
  }

  function handleOpenChildTask(child: Task) {
    setPreviousTask(selectedTask)
    setSelectedTask(child)
  }

  function handleBackFromChild() {
    const liveParent = listTasks.find((t) => t.id === previousTask?.id) ?? previousTask
    setSelectedTask(liveParent)
    setPreviousTask(null)
  }

  async function handleDeleteList() {
    await removeList(listId)
    onNavigate({ type: 'dashboard' })
  }

  async function handleExport() {
    const md = listToMarkdown(list?.title ?? 'Lista', listTasks)
    await navigator.clipboard.writeText(md)
    setCopyFeedback(true)
    setTimeout(() => setCopyFeedback(false), 2000)
  }

  return (
    <div className="flex h-full">
      <div className="flex-1 overflow-y-auto">
        {/* Header */}
        <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-800 flex items-center gap-3">
          {list?.color && (
            <span
              className="w-3 h-3 rounded-full shrink-0"
              style={{ backgroundColor: list.color }}
            />
          )}

          {editingTitle ? (
            <input
              ref={titleInputRef}
              autoFocus
              className="text-xl font-semibold bg-transparent border-b border-indigo-400 outline-none text-gray-900 dark:text-gray-100 flex-1"
              value={titleDraft}
              onChange={(e) => setTitleDraft(e.target.value)}
              onBlur={handleSaveTitle}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleSaveTitle()
                if (e.key === 'Escape') setEditingTitle(false)
              }}
            />
          ) : (
            <h2
              className="text-xl font-semibold text-gray-900 dark:text-gray-100 cursor-pointer hover:text-indigo-600 dark:hover:text-indigo-400 transition-colors flex-1"
              onClick={handleStartEditTitle}
              title="Click to rename"
            >
              {list?.title ?? 'List'}
            </h2>
          )}

          <button
            onClick={handleExport}
            className="text-indigo-500 hover:text-indigo-700 hover:bg-indigo-50 dark:hover:bg-indigo-900/20 rounded px-2 py-1 text-xs font-medium transition-colors shrink-0"
            title="Eksportuj jako Markdown"
          >
            {copyFeedback ? 'Skopiowano!' : 'Eksportuj MD'}
          </button>

          {confirmDeleteList ? (
            <div className="flex items-center gap-1 shrink-0">
              <button
                onClick={handleDeleteList}
                className="text-xs px-2 py-1 bg-red-500 hover:bg-red-600 text-white rounded transition-colors"
              >
                Delete
              </button>
              <button
                onClick={() => setConfirmDeleteList(false)}
                className="text-xs px-2 py-1 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 rounded hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              onClick={() => setConfirmDeleteList(true)}
              className="text-red-500 hover:text-red-700 hover:bg-red-50 dark:hover:bg-red-900/20 rounded px-2 py-1 text-xs font-medium transition-colors shrink-0"
              title="Delete list"
            >
              Delete list
            </button>
          )}
        </div>

        {/* Tag filter bar */}
        {allTags.length > 0 && (
          <div className="px-4 py-2 flex flex-wrap gap-1.5 border-b border-gray-100 dark:border-gray-800">
            {allTags.map((tag) => (
              <button
                key={tag.name}
                onClick={() => setActiveTagFilter(activeTagFilter === tag.name ? null : tag.name)}
                className={`text-xs px-2 py-0.5 rounded-full transition-colors ${
                  activeTagFilter === tag.name
                    ? 'bg-indigo-500 text-white'
                    : 'bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300 hover:bg-indigo-200 dark:hover:bg-indigo-800'
                }`}
              >
                {tag.name}
              </button>
            ))}
          </div>
        )}

        {/* Incomplete tasks */}
        {incomplete.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            onOpen={setSelectedTask}
            onToggleComplete={handleToggleComplete}
            onDelete={handleDeleteTask}
          />
        ))}

        {/* Add task row */}
        {isAdding ? (
          <form
            onSubmit={(e) => { e.preventDefault(); handleAdd() }}
            className="px-4 py-2 border-b border-gray-100 dark:border-gray-800"
          >
            <input
              autoFocus
              className="w-full text-sm px-3 py-2 border border-indigo-300 dark:border-indigo-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none"
              placeholder="Task title…"
              value={addingTitle}
              onChange={(e) => setAddingTitle(e.target.value)}
              onBlur={handleAdd}
              onKeyDown={(e) => e.key === 'Escape' && setIsAdding(false)}
            />
          </form>
        ) : (
          <button
            className="w-full text-left px-4 py-3 text-sm text-gray-400 dark:text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800/50 border-b border-gray-100 dark:border-gray-800 transition-colors"
            onClick={() => setIsAdding(true)}
          >
            + Add task
          </button>
        )}

        {/* Completed tasks */}
        {completed.length > 0 && (
          <div className="mt-4">
            <button
              className="w-full text-left px-4 py-2 text-sm text-gray-500 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800/50 flex items-center gap-2"
              onClick={() => setShowCompleted((v) => !v)}
            >
              <span>{showCompleted ? '▾' : '▸'}</span>
              Completed ({completed.length})
            </button>
            {showCompleted && completed.map((task) => (
              <TaskCard
                key={task.id}
                task={task}
                onOpen={setSelectedTask}
                onToggleComplete={handleToggleComplete}
                onDelete={handleDeleteTask}
              />
            ))}
          </div>
        )}
      </div>

      {selectedTask && (() => {
        const liveTask = listTasks.find((t) => t.id === selectedTask.id) ?? selectedTask
        return (
          <TaskEditor
            task={liveTask}
            listId={listId}
            onClose={() => { setSelectedTask(null); setPreviousTask(null) }}
            parentTask={previousTask ?? undefined}
            onBack={previousTask ? handleBackFromChild : undefined}
            onOpenChildTask={handleOpenChildTask}
          />
        )
      })()}
    </div>
  )
}
