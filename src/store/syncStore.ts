import { create } from 'zustand'
import { api } from '../lib/tauri'
import type { SyncConflict, SyncStatus } from '../types'

interface SyncStore {
  status: SyncStatus | null
  conflicts: SyncConflict[]
  syncing: boolean
  /** Last failure, shown in Settings rather than thrown away. */
  error: string | null
  load: () => Promise<void>
  setFolder: (path: string) => Promise<void>
  disable: () => Promise<void>
  syncNow: () => Promise<void>
  resolve: (id: number, restoreDiscarded: boolean) => Promise<void>
  renameDevice: (name: string) => Promise<void>
}

export const useSyncStore = create<SyncStore>((set, get) => ({
  status: null,
  conflicts: [],
  syncing: false,
  error: null,

  load: async () => {
    const [status, conflicts] = await Promise.all([api.getSyncStatus(), api.getConflicts()])
    set({ status, conflicts })
  },

  setFolder: async (path) => {
    set({ syncing: true, error: null })
    try {
      await api.setSyncFolder(path)
      await get().load()
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ syncing: false })
    }
  },

  disable: async () => {
    await api.disableSync()
    await get().load()
  },

  syncNow: async () => {
    set({ syncing: true, error: null })
    try {
      await api.syncNow()
      await get().load()
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ syncing: false })
    }
  },

  resolve: async (id, restoreDiscarded) => {
    await api.resolveConflict(id, restoreDiscarded)
    set((s) => ({ conflicts: s.conflicts.filter((c) => c.id !== id) }))
  },

  renameDevice: async (name) => {
    await api.setDeviceName(name)
    await get().load()
  },
}))

/** Conflict values travel as JSON so that null and "null" stay distinct. */
export function readValue(raw: string): string {
  try {
    const parsed = JSON.parse(raw)
    if (parsed === null || parsed === '') return '(empty)'
    return String(parsed)
  } catch {
    return raw
  }
}
