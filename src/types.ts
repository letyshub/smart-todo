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
  parent_task_id: number | null
  is_subtask: boolean
  status: 'todo' | 'inprogress' | 'done'
  tags: Tag[]
  total_seconds: number
}

export interface DashboardData {
  overdue: Task[]
  high_priority: Task[]
  upcoming: Task[]
}

export interface ProductivityStats {
  tasks_completed_week: number
  total_seconds_week: number
  on_time_count: number
  late_count: number
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
  /** Always local. Sharing between machines is the sync folder's job. */
  database_path: string
  sidebar_width: number | null
  task_editor_width: number | null
}

export interface SyncPeer {
  device_id: string
  name: string
  platform: string
  last_seen: string
}

export interface SyncStatus {
  folder: string | null
  device_name: string
  peers: SyncPeer[]
  open_conflicts: number
  waiting: number
}

export interface SyncReport {
  pushed: number
  applied: number
  conflicts: number
  waiting: number
}

/** One field two machines changed independently. */
export interface SyncConflict {
  id: number
  entity: string
  uuid: string
  field: string
  /** Name of the row the conflict is on, e.g. the task title. */
  subject: string
  /** JSON-encoded value that won. */
  kept: string
  /** JSON-encoded value that was overridden on this machine. */
  discarded: string
  detected_at: string
}

export interface StartTimerResult {
  stopped_task_id: number | null
}
