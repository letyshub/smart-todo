import { useEffect, useState } from 'react'
import Sidebar from './components/Sidebar'
import Dashboard from './pages/Dashboard'
import ListDetail from './pages/ListDetail'
import Settings from './pages/Settings'
import { useListsStore } from './store/listsStore'
import { useSettingsStore } from './store/settingsStore'

export type View = { type: 'dashboard' } | { type: 'list'; id: number } | { type: 'settings' }

export default function App() {
  const [view, setView] = useState<View>({ type: 'dashboard' })
  const loadLists = useListsStore((s) => s.load)
  const loadSettings = useSettingsStore((s) => s.load)

  useEffect(() => {
    loadLists()
    loadSettings()
  }, [])

  return (
    <div className="flex h-screen overflow-hidden bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-100">
      <Sidebar view={view} onNavigate={setView} />
      <main className="flex-1 overflow-y-auto">
        {view.type === 'dashboard' && <Dashboard onNavigate={setView} />}
        {view.type === 'list' && <ListDetail listId={view.id} onNavigate={setView} />}
        {view.type === 'settings' && <Settings />}
      </main>
    </div>
  )
}
