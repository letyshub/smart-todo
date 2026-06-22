import { create } from 'zustand'
import { api } from '../lib/tauri'
import type { Task, DashboardData } from '../types'

interface TasksStore {
  tasks: Record<number, Task[]>
  dashboard: DashboardData | null
  loadList: (listId: number) => Promise<void>
  loadDashboard: () => Promise<void>
  create: (listId: number, title: string) => Promise<Task>
  update: (id: number, listId: number, fields: Parameters<typeof api.updateTask>[1]) => Promise<void>
  remove: (id: number, listId: number) => Promise<void>
  setTags: (taskId: number, listId: number, tagNames: string[]) => Promise<void>
}

export const useTasksStore = create<TasksStore>((set) => ({
  tasks: {},
  dashboard: null,
  loadList: async (listId) => {
    const tasks = await api.getTasks(listId)
    set((s) => ({ tasks: { ...s.tasks, [listId]: tasks } }))
  },
  loadDashboard: async () => {
    const dashboard = await api.getDashboardTasks()
    set({ dashboard })
  },
  create: async (listId, title) => {
    const task = await api.createTask(listId, title)
    set((s) => ({ tasks: { ...s.tasks, [listId]: [...(s.tasks[listId] ?? []), task] } }))
    return task
  },
  update: async (id, listId, fields) => {
    const updated = await api.updateTask(id, fields)
    set((s) => ({
      tasks: {
        ...s.tasks,
        [listId]: (s.tasks[listId] ?? []).map((t) => (t.id === id ? updated : t)),
      },
    }))
  },
  remove: async (id, listId) => {
    await api.deleteTask(id)
    set((s) => ({
      tasks: {
        ...s.tasks,
        [listId]: (s.tasks[listId] ?? []).filter((t) => t.id !== id),
      },
    }))
  },
  setTags: async (taskId, listId, tagNames) => {
    const tags = await api.setTaskTags(taskId, tagNames)
    set((s) => ({
      tasks: {
        ...s.tasks,
        [listId]: (s.tasks[listId] ?? []).map((t) =>
          t.id === taskId ? { ...t, tags } : t
        ),
      },
    }))
  },
}))
