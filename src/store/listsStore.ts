import { create } from 'zustand'
import { api } from '../lib/tauri'
import type { List } from '../types'
import { useTasksStore } from './tasksStore'

interface ListsStore {
  lists: List[]
  loading: boolean
  load: () => Promise<void>
  create: (title: string, color?: string) => Promise<List>
  update: (id: number, title?: string, color?: string) => Promise<void>
  remove: (id: number) => Promise<void>
}

export const useListsStore = create<ListsStore>((set) => ({
  lists: [],
  loading: false,
  load: async () => {
    set({ loading: true })
    const lists = await api.getLists()
    set({ lists, loading: false })
  },
  create: async (title, color) => {
    const list = await api.createList(title, color)
    set((s) => ({ lists: [...s.lists, list] }))
    return list
  },
  update: async (id, title, color) => {
    const updated = await api.updateList(id, title, color)
    set((s) => ({ lists: s.lists.map((l) => (l.id === id ? updated : l)) }))
  },
  remove: async (id) => {
    await api.deleteList(id)
    set((s) => ({ lists: s.lists.filter((l) => l.id !== id) }))
    const { loadDashboard, tasks } = useTasksStore.getState()
    const withoutDeleted = Object.fromEntries(
      Object.entries(tasks).filter(([k]) => Number(k) !== id)
    )
    useTasksStore.setState({ tasks: withoutDeleted })
    await loadDashboard()
  },
}))
