import { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import Sidebar from './components/Sidebar'
import SyncConflicts from './components/SyncConflicts'
import Dashboard from './pages/Dashboard'
import ListDetail from './pages/ListDetail'
import Settings from './pages/Settings'
import { useListsStore } from './store/listsStore'
import { useSettingsStore } from './store/settingsStore'
import { useSyncStore } from './store/syncStore'
import { useTasksStore } from './store/tasksStore'

export type View = { type: 'dashboard' } | { type: 'list'; id: number } | { type: 'settings' }

export default function App() {
  const [view, setView] = useState<View>({ type: 'dashboard' })
  const loadLists = useListsStore((s) => s.load)
  const loadSettings = useSettingsStore((s) => s.load)
  const loadSync = useSyncStore((s) => s.load)
  const loadDashboard = useTasksStore((s) => s.loadDashboard)
  const loadList = useTasksStore((s) => s.loadList)

  useEffect(() => {
    loadLists()
    loadSettings()
    loadSync()
  }, [loadLists, loadSettings, loadSync])

  // Changes from the other machine arrive in the background, so the screen has
  // to refresh itself rather than wait for the user to navigate somewhere.
  useEffect(() => {
    const unlisten = listen('sync:changed', () => {
      loadLists()
      loadSync()
      if (view.type === 'list') loadList(view.id)
      else loadDashboard()
    })
    return () => {
      unlisten.then((off) => off()).catch(() => {})
    }
  }, [loadLists, loadSync, loadDashboard, loadList, view])

  return (
    <div className="flex h-screen overflow-hidden bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100">
      <Sidebar view={view} onNavigate={setView} />
      <main className="flex-1 overflow-y-auto flex flex-col">
        <SyncConflicts />
        <div className="flex-1">
          {view.type === 'dashboard' && <Dashboard onNavigate={setView} />}
          {view.type === 'list' && <ListDetail listId={view.id} onNavigate={setView} />}
          {view.type === 'settings' && <Settings />}
        </div>
      </main>
    </div>
  )
}
