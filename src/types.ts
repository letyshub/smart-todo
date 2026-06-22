export interface List {
  id: number
  title: string
  color: string | null
  position: number
  created_at: string
}

export interface Tag {
  id: number
  name: string
  color: string | null
}

export interface Task {
  id: number
  list_id: number
  title: string
  description: string | null
  priority: 'normal' | 'high'
  due_date: string | null
  completed: boolean
  completed_at: string | null
  position: number
  created_at: string
  updated_at: string
  tags: Tag[]
  total_seconds: number
}

export interface DashboardData {
  overdue: Task[]
  high_priority: Task[]
  upcoming: Task[]
}

export interface TimerSession {
  id: number
  task_id: number
  started_at: string
  stopped_at: string | null
  duration_seconds: number | null
}

export interface ActiveTimer {
  task_id: number
  elapsed_seconds: number
}

export interface Settings {
  theme: 'light' | 'dark' | 'system'
  data_dir: string | null
}

export interface StartTimerResult {
  stopped_task_id: number | null
}
