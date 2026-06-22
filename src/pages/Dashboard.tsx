import { useEffect, useState } from 'react'
import type { Task } from '../types'
import type { View } from '../App'
import { useTasksStore } from '../store/tasksStore'
import TaskCard from '../components/TaskCard'
import TaskEditor from '../components/TaskEditor'

interface Props {
  onNavigate: (v: View) => void
}

export default function Dashboard({ onNavigate: _onNavigate }: Props) {
  const { dashboard, loadDashboard, update } = useTasksStore()
  const [selectedTask, setSelectedTask] = useState<Task | null>(null)

  useEffect(() => {
    loadDashboard()
  }, [loadDashboard])

  async function handleToggleComplete(task: Task) {
    await update(task.id, task.list_id, { completed: !task.completed })
    loadDashboard()
  }

  function handleOpenTask(task: Task) {
    setSelectedTask(task)
  }

  if (!dashboard) {
    return (
      <div className="flex items-center justify-center h-full text-gray-400 text-sm">
        Loading…
      </div>
    )
  }

  return (
    <div className="flex h-full">
      <div className="flex-1 overflow-y-auto">
        <div className="px-6 py-4 border-b border-gray-100 dark:border-gray-800">
          <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100">Dashboard</h2>
        </div>

        <Section
          title="Due Today / Overdue"
          tasks={dashboard.overdue}
          emptyMessage="No overdue or due-today tasks."
          onOpen={handleOpenTask}
          onToggleComplete={handleToggleComplete}
        />

        <Section
          title="High Priority"
          tasks={dashboard.high_priority}
          emptyMessage="No high-priority tasks."
          onOpen={handleOpenTask}
          onToggleComplete={handleToggleComplete}
        />

        <Section
          title="Upcoming"
          tasks={dashboard.upcoming}
          emptyMessage="No upcoming tasks in the next 7 days."
          onOpen={handleOpenTask}
          onToggleComplete={handleToggleComplete}
        />
      </div>

      {selectedTask && (
        <TaskEditor
          task={selectedTask}
          listId={selectedTask.list_id}
          onClose={() => setSelectedTask(null)}
        />
      )}
    </div>
  )
}

interface SectionProps {
  title: string
  tasks: Task[]
  emptyMessage: string
  onOpen: (task: Task) => void
  onToggleComplete: (task: Task) => void
}

function Section({ title, tasks, emptyMessage, onOpen, onToggleComplete }: SectionProps) {
  return (
    <div className="mb-6">
      <div className="px-6 py-2 border-b border-gray-100 dark:border-gray-800">
        <h3 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide">
          {title}
          {tasks.length > 0 && (
            <span className="ml-2 text-xs font-normal text-gray-400">({tasks.length})</span>
          )}
        </h3>
      </div>
      {tasks.length === 0 ? (
        <p className="px-6 py-3 text-sm text-gray-400 dark:text-gray-500">{emptyMessage}</p>
      ) : (
        tasks.map((task) => (
          <TaskCard
            key={task.id}
            task={task}
            onOpen={onOpen}
            onToggleComplete={onToggleComplete}
          />
        ))
      )}
    </div>
  )
}
