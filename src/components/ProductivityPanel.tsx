import type { ProductivityStats } from '../types'
import { formatTotal } from '../lib/timeUtils'

interface Props {
  stats: ProductivityStats
}

export default function ProductivityPanel({ stats }: Props) {
  const { tasks_completed_week, total_seconds_week, on_time_count, late_count } = stats
  const totalWithDate = on_time_count + late_count
  const onTimeDisplay = totalWithDate === 0 ? '—' : `${on_time_count} / ${totalWithDate}`
  const timeDisplay = formatTotal(total_seconds_week) || '0m'

  return (
    <div className="w-56 border-l border-gray-100 dark:border-gray-800 flex flex-col flex-shrink-0">
      <div className="px-4 py-4 border-b border-gray-100 dark:border-gray-800">
        <h3 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide">
          Ten tydzień
        </h3>
      </div>
      <div className="flex flex-col gap-1 px-4 py-4">
        <StatTile value={String(tasks_completed_week)} label="ukończone zadania" />
        <StatTile value={timeDisplay} label="zarejestrowany czas" />
        <StatTile value={onTimeDisplay} label="ukończonych na czas" />
      </div>
    </div>
  )
}

function StatTile({ value, label }: { value: string; label: string }) {
  return (
    <div className="py-3 border-b border-gray-100 dark:border-gray-800 last:border-0">
      <div className="text-2xl font-bold text-gray-900 dark:text-gray-100">{value}</div>
      <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{label}</div>
    </div>
  )
}
