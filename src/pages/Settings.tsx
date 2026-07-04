import { useState, useEffect } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { getVersion } from '@tauri-apps/api/app'
import { useSettingsStore } from '../store/settingsStore'

type Theme = 'light' | 'dark' | 'system'

const THEME_OPTIONS: { value: Theme; label: string }[] = [
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'system', label: 'System' },
]

export default function Settings() {
  const { settings, setTheme, changeDataDir } = useSettingsStore()
  const [changing, setChanging] = useState(false)
  const [version, setVersion] = useState<string | null>(null)

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion(null))
  }, [])

  async function handleChangeDir() {
    setChanging(true)
    try {
      const selected = await open({ directory: true, multiple: false })
      if (selected && typeof selected === 'string') {
        await changeDataDir(selected)
      }
    } finally {
      setChanging(false)
    }
  }

  return (
    <div className="max-w-xl mx-auto px-6 py-8">
      <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100 mb-6">Settings</h2>

      {/* Theme */}
      <section className="mb-8">
        <h3 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">
          Theme
        </h3>
        <div className="flex gap-1 p-1 bg-gray-100 dark:bg-gray-800 rounded-lg w-fit">
          {THEME_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setTheme(opt.value)}
              className={`px-4 py-1.5 text-sm rounded-md transition-colors ${
                settings?.theme === opt.value
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm font-medium'
                  : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200'
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </section>

      {/* Data directory */}
      <section>
        <h3 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">
          Data Directory
        </h3>
        <p className="text-xs text-gray-400 dark:text-gray-500 mb-3">
          Your todo data is stored as a SQLite file. Point it at an iCloud Drive or OneDrive
          folder to sync across devices — the OS sync client handles the rest.
        </p>
        <div className="flex items-center gap-3">
          <code className="flex-1 text-xs bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 px-3 py-2 rounded border border-gray-200 dark:border-gray-700 truncate">
            {settings?.data_dir ?? 'Default (app data folder)'}
          </code>
          <button
            type="button"
            onClick={handleChangeDir}
            disabled={changing}
            className="text-sm px-3 py-2 bg-indigo-600 text-white rounded hover:bg-indigo-700 disabled:opacity-50 transition-colors shrink-0"
          >
            {changing ? 'Selecting…' : 'Change…'}
          </button>
        </div>
      </section>
      {/* About */}
      {version && (
        <p className="mt-12 text-xs text-gray-400 dark:text-gray-600">
          Smart Todo v{version}
        </p>
      )}
    </div>
  )
}
