import type { Task } from '../types'

function formatPriority(priority: Task['priority']): string {
  return priority === 'high' ? ' ⚡ wysoki priorytet' : ''
}

function formatDueDate(dueDate: string | null): string {
  if (!dueDate) return ''
  return ` · termin: ${dueDate}`
}

function formatTags(tags: Task['tags']): string {
  if (!tags.length) return ''
  return ' ' + tags.map((t) => `#${t.name}`).join(' ')
}

function formatStatus(status: Task['status']): string {
  if (status === 'inprogress') return ' · w toku'
  return ''
}

function taskLine(task: Task): string[] {
  const checkbox = task.completed ? '- [x]' : '- [ ]'
  const title = task.completed ? `~~${task.title}~~` : task.title
  const meta = [
    formatPriority(task.priority),
    formatDueDate(task.due_date),
    formatStatus(task.status),
    formatTags(task.tags),
  ].join('')

  const lines: string[] = [`${checkbox} ${title}${meta}`]

  if (task.description) {
    task.description
      .split('\n')
      .forEach((line) => lines.push(`  ${line}`))
  }

  return lines
}

export function listToMarkdown(listTitle: string, tasks: Task[]): string {
  const incomplete = tasks.filter((t) => !t.completed)
  const completed = tasks.filter((t) => t.completed)

  const lines: string[] = [`# ${listTitle}`, '']

  if (incomplete.length) {
    incomplete.forEach((t) => lines.push(...taskLine(t)))
  }

  if (completed.length) {
    if (incomplete.length) lines.push('')
    lines.push('## Ukończone', '')
    completed.forEach((t) => lines.push(...taskLine(t)))
  }

  if (!tasks.length) {
    lines.push('_Brak zadań._')
  }

  return lines.join('\n')
}
