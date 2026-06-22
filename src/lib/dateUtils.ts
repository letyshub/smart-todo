export function today(): string {
  return new Date().toISOString().split('T')[0]
}

export function isOverdue(dueDate: string | null): boolean {
  if (!dueDate) return false
  return dueDate < today()
}

export function isDueToday(dueDate: string | null): boolean {
  if (!dueDate) return false
  return dueDate === today()
}

export function formatDueDate(dueDate: string): string {
  const d = new Date(dueDate + 'T00:00:00')
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}
