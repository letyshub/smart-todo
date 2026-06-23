import { useEffect, useState } from 'react'
import type { Task } from '../types'
import type { View } from '../App'
import { useListsStore } from '../store/listsStore'
import { useTasksStore } from '../store/tasksStore'
import TaskCard from '../components/TaskCard'
import TaskEditor from '../components/TaskEditor'

interface Props {
  listId: number
  onNavigate: (v: View) => void
}

export default function ListDetail({ listId }: Props) {
  const lists = useListsStore((s) => s.lists)
  const { tasks, loadList, create, update } = useTasksStore()
  const list = lists.find((l) => l.id === listId)
  const listTasks = tasks[listId] ?? []

  const [selectedTask, setSelectedTask] = useState<Task | null>(null)
  const [showCompleted, setShowCompleted] = useState(false)
  const [addingTitle, setAddingTitle] = useState('')
  const [isAdding, setIsAdding] = useState(false)

  useEffect(() => {
    loadList(listId)
  }, [listId, loadList])

  const incomplete = listTasks.filter((t) => !t.completed)
  const completed = listTasks.filter((t) => t.completed)

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

  return (
    <div className="flex h-full">
      {/* Task list */}
      <div className="flex-1 overflow-y-auto">
        {/* Header */}
        <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-800 flex items-center gap-3">
          {list?.color && (
            <span
              className="w-3 h-3 rounded-full shrink-0"
              style={{ backgroundColor: list.color }}
            />
          )}
          <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
            {list?.title ?? 'List'}
          </h2>
        </div>

        {/* Incomplete tasks */}
        {incomplete.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            onOpen={setSelectedTask}
            onToggleComplete={handleToggleComplete}
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

        {/* Completed tasks disclosure */}
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
              />
            ))}
          </div>
        )}
      </div>

      {/* Derive live task from store so TaskEditor always sees fresh data */}
      {selectedTask && (() => {
        const liveTask = listTasks.find(t => t.id === selectedTask.id) ?? selectedTask
        return (
          <TaskEditor
            task={liveTask}
            listId={listId}
            onClose={() => setSelectedTask(null)}
          />
        )
      })()}
    </div>
  )
}
