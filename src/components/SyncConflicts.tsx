import { useState } from 'react'
import { readValue, useSyncStore } from '../store/syncStore'
import type { SyncConflict } from '../types'

const FIELD_LABELS: Record<string, string> = {
  title: 'title',
  name: 'name',
  description: 'description',
  due_date: 'due date',
  priority: 'priority',
  status: 'status',
  completed: 'completed',
  position: 'position',
  color: 'colour',
  list_uuid: 'list',
  parent_task_uuid: 'parent task',
}

function label(field: string) {
  return FIELD_LABELS[field] ?? field.replace(/_/g, ' ')
}

function ConflictRow({
  conflict,
  onResolve,
}: {
  conflict: SyncConflict
  onResolve: (restoreDiscarded: boolean) => void
}) {
  return (
    <li className="py-3 border-t border-amber-200 dark:border-amber-800/60 first:border-t-0">
      <p className="text-sm text-gray-800 dark:text-gray-200">
        <span className="font-medium">{conflict.subject}</span>
        <span className="text-gray-500 dark:text-gray-400"> — {label(conflict.field)}</span>
      </p>
      <div className="mt-2 grid gap-2 sm:grid-cols-2">
        <div className="rounded border border-gray-200 dark:border-gray-700 px-3 py-2">
          <p className="text-[11px] uppercase tracking-wide text-gray-400 dark:text-gray-500">
            Kept (from your other machine)
          </p>
          <p className="text-sm text-gray-800 dark:text-gray-200 break-words">
            {readValue(conflict.kept)}
          </p>
        </div>
        <div className="rounded border border-gray-200 dark:border-gray-700 px-3 py-2">
          <p className="text-[11px] uppercase tracking-wide text-gray-400 dark:text-gray-500">
            Yours, overridden
          </p>
          <p className="text-sm text-gray-800 dark:text-gray-200 break-words">
            {readValue(conflict.discarded)}
          </p>
        </div>
      </div>
      <div className="mt-2 flex gap-2">
        <button
          type="button"
          onClick={() => onResolve(false)}
          className="text-xs px-2.5 py-1.5 rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
        >
          Keep this
        </button>
        <button
          type="button"
          onClick={() => onResolve(true)}
          className="text-xs px-2.5 py-1.5 rounded bg-amber-600 text-white hover:bg-amber-700"
        >
          Restore mine
        </button>
      </div>
    </li>
  )
}

/**
 * Shown only on the machine whose edit was overridden — the one place where
 * putting the old value back means anything.
 */
export default function SyncConflicts() {
  const conflicts = useSyncStore((s) => s.conflicts)
  const resolve = useSyncStore((s) => s.resolve)
  const [open, setOpen] = useState(false)

  if (conflicts.length === 0) return null

  return (
    <div className="border-b border-amber-200 dark:border-amber-800/60 bg-amber-50 dark:bg-amber-950/30">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center gap-2 px-6 py-2.5 text-left text-sm text-amber-900 dark:text-amber-200"
      >
        <span aria-hidden>⚠</span>
        <span className="flex-1">
          {conflicts.length === 1
            ? '1 edit was overridden by your other machine'
            : `${conflicts.length} edits were overridden by your other machine`}
        </span>
        <span className="text-xs text-amber-700 dark:text-amber-300">
          {open ? 'Hide' : 'Review'}
        </span>
      </button>
      {open && (
        <ul className="px-6 pb-4">
          {conflicts.map((c) => (
            <ConflictRow
              key={c.id}
              conflict={c}
              onResolve={(restore) => resolve(c.id, restore)}
            />
          ))}
        </ul>
      )}
    </div>
  )
}
