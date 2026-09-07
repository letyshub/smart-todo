import { create } from 'zustand'
import { api } from '../lib/tauri'
import type { Settings } from '../types'

interface SettingsStore {
  settings: Settings | null
  load: () => Promise<void>
  setTheme: (theme: 'light' | 'dark' | 'system') => Promise<void>
  setSidebarWidth: (width: number) => Promise<void>
  setTaskEditorWidth: (width: number) => Promise<void>
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: null,
  load: async () => {
    const settings = await api.getSettings()
    set({ settings })
    applyTheme(settings.theme)
  },
  setTheme: async (theme) => {
    await api.setSetting('theme', theme)
    set((s) => ({ settings: s.settings ? { ...s.settings, theme } : null }))
    applyTheme(theme)
  },
  setSidebarWidth: async (width) => {
    set((s) => ({ settings: s.settings ? { ...s.settings, sidebar_width: width } : null }))
    await api.setSetting('sidebar_width', String(width))
  },
  setTaskEditorWidth: async (width) => {
    set((s) => ({ settings: s.settings ? { ...s.settings, task_editor_width: width } : null }))
    await api.setSetting('task_editor_width', String(width))
  },
}))

function applyTheme(theme: 'light' | 'dark' | 'system') {
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  const isDark = theme === 'dark' || (theme === 'system' && prefersDark)
  document.documentElement.classList.toggle('dark', isDark)
}
