import { useState, useEffect } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { getVersion } from '@tauri-apps/api/app'
import { useSettingsStore } from '../store/settingsStore'
import { useSyncStore } from '../store/syncStore'

type Theme = 'light' | 'dark' | 'system'

const THEME_OPTIONS: { value: Theme; label: string }[] = [
  { value: 'light', label: 'Light' },
  { value: 'dark', label: 'Dark' },
  { value: 'system', label: 'System' },
]

export default function Settings() {
  const { settings, setTheme } = useSettingsStore()
  const { status, syncing, error, load, setFolder, disable, syncNow } = useSyncStore()
  const [version, setVersion] = useState<string | null>(null)

  useEffect(() => {
    getVersion().then(setVersion).catch(() => setVersion(null))
    load()
  }, [load])

  async function handleChooseFolder() {
    const selected = await open({ directory: true, multiple: false })
    if (selected && typeof selected === 'string') {
      await setFolder(selected)
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

      {/* Sync */}
      <section className="mb-8">
        <h3 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">
          Sync
        </h3>
        <p className="text-xs text-gray-400 dark:text-gray-500 mb-3">
          Pick a folder inside OneDrive, iCloud Drive or Dropbox. Each machine keeps its own
          database and publishes only a log of changes there, so nothing is ever written by
          two machines at once. Editing on both is fine, offline included.
        </p>

        {status?.folder ? (
          <>
            <div className="flex items-center gap-3">
              <code className="flex-1 text-xs bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 px-3 py-2 rounded border border-gray-200 dark:border-gray-700 truncate">
                {status.folder}
              </code>
              <button
                type="button"
                onClick={syncNow}
                disabled={syncing}
                className="text-sm px-3 py-2 bg-indigo-600 text-white rounded hover:bg-indigo-700 disabled:opacity-50 transition-colors shrink-0"
              >
                {syncing ? 'Syncing…' : 'Sync now'}
              </button>
            </div>
            <p className="mt-2 text-xs text-gray-400 dark:text-gray-500">
              This machine is <span className="font-medium">{status.device_name}</span>.
              {status.peers.length === 0
                ? ' No other machine has published here yet.'
                : ` Also here: ${status.peers.map((p) => p.name).join(', ')}.`}
              {status.waiting > 0 && ` ${status.waiting} change(s) waiting on data still on its way.`}
            </p>
            <button
              type="button"
              onClick={disable}
              className="mt-3 text-xs text-gray-500 dark:text-gray-400 underline hover:text-gray-700 dark:hover:text-gray-200"
            >
              Stop syncing
            </button>
          </>
        ) : (
          <button
            type="button"
            onClick={handleChooseFolder}
            disabled={syncing}
            className="text-sm px-3 py-2 bg-indigo-600 text-white rounded hover:bg-indigo-700 disabled:opacity-50 transition-colors"
          >
            {syncing ? 'Setting up…' : 'Choose sync folder…'}
          </button>
        )}

        {error && (
          <p className="mt-3 text-xs text-red-600 dark:text-red-400">{error}</p>
        )}
      </section>

      {/* Database location */}
      <section>
        <h3 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-3">
          Database
        </h3>
        <p className="text-xs text-gray-400 dark:text-gray-500 mb-3">
          Kept on this machine on purpose. A SQLite file living in a cloud-synced folder gets
          corrupted, because the sync client copies its several files independently.
        </p>
        <code className="block text-xs bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 px-3 py-2 rounded border border-gray-200 dark:border-gray-700 truncate">
          {settings?.database_path ?? '—'}
        </code>
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
