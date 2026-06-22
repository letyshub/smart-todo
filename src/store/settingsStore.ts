import { create } from 'zustand'
import { api } from '../lib/tauri'
import type { Settings } from '../types'

interface SettingsStore {
  settings: Settings | null
  load: () => Promise<void>
  setTheme: (theme: 'light' | 'dark' | 'system') => Promise<void>
  changeDataDir: (path: string) => Promise<void>
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
  changeDataDir: async (path) => {
    await api.changeDataDir(path)
    set((s) => ({ settings: s.settings ? { ...s.settings, data_dir: path } : null }))
  },
}))

function applyTheme(theme: 'light' | 'dark' | 'system') {
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
  const isDark = theme === 'dark' || (theme === 'system' && prefersDark)
  document.documentElement.classList.toggle('dark', isDark)
}
