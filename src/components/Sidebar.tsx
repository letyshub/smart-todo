import { useState } from 'react'
import type { View } from '../App'
import { useListsStore } from '../store/listsStore'

interface Props {
  view: View
  onNavigate: (v: View) => void
}

export default function Sidebar({ view, onNavigate }: Props) {
  const { lists, create } = useListsStore()

  const [adding, setAdding] = useState(false)
  const [newTitle, setNewTitle] = useState('')

  async function handleAddList() {
    const title = newTitle.trim()
    if (!title) { setAdding(false); return }
    const list = await create(title)
    setNewTitle('')
    setAdding(false)
    onNavigate({ type: 'list', id: list.id })
  }

  const navItem = (active: boolean) =>
    `flex items-center gap-2 px-3 py-2 rounded-lg text-sm cursor-pointer transition-colors ${
      active
        ? 'bg-indigo-100 dark:bg-indigo-900 text-indigo-700 dark:text-indigo-300 font-medium'
        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'
    }`

  return (
    <aside className="w-60 shrink-0 border-r border-gray-200 dark:border-gray-700 flex flex-col h-screen p-3 gap-1">
      <h1 className="text-lg font-bold px-2 py-3 text-gray-900 dark:text-gray-100">Smart Todo</h1>

      <button
        className={navItem(view.type === 'dashboard')}
        onClick={() => onNavigate({ type: 'dashboard' })}
      >
        <span aria-hidden="true">📋</span> Dashboard
      </button>

      <hr className="border-gray-200 dark:border-gray-700 my-1" />

      <div className="flex-1 overflow-y-auto space-y-0.5">
        {lists.map((list) => (
          <button
            key={list.id}
            className={`${navItem(view.type === 'list' && view.id === list.id)} w-full`}
            onClick={() => onNavigate({ type: 'list', id: list.id })}
          >
            <span
              className="w-2.5 h-2.5 rounded-full shrink-0"
              style={{ backgroundColor: list.color ?? '#6366f1' }}
            />
            <span className="truncate flex-1 text-left">{list.title}</span>
          </button>
        ))}

        {adding ? (
          <form
            onSubmit={(e) => { e.preventDefault(); handleAddList() }}
            className="flex gap-1 mt-1"
          >
            <input
              autoFocus
              className="flex-1 text-sm px-2 py-1 border border-gray-300 dark:border-gray-600 rounded bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 outline-none"
              aria-label="List name"
              placeholder="List name…"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onBlur={handleAddList}
              onKeyDown={(e) => e.key === 'Escape' && setAdding(false)}
            />
          </form>
        ) : (
          <button
            className="flex items-center gap-2 px-3 py-2 text-sm text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 w-full rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            onClick={() => setAdding(true)}
          >
            <span>+</span> New List
          </button>
        )}
      </div>

      <hr className="border-gray-200 dark:border-gray-700 my-1" />

      <button
        className={navItem(view.type === 'settings')}
        onClick={() => onNavigate({ type: 'settings' })}
      >
        <span aria-hidden="true">⚙</span> Settings
      </button>
    </aside>
  )
}
