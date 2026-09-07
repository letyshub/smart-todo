import { invoke } from '@tauri-apps/api/core'
import type {
  List, Task, Tag, DashboardData, TimerSession, ActiveTimer,
  Settings, StartTimerResult, ProductivityStats,
  SyncStatus, SyncReport, SyncConflict,
} from '../types'

export const api = {
  getLists: () => invoke<List[]>('get_lists'),
  createList: (title: string, color?: string) =>
    invoke<List>('create_list', { title, color }),
  updateList: (id: number, title?: string, color?: string, position?: number) =>
    invoke<List>('update_list', { id, title, color, position }),
  deleteList: (id: number) => invoke<void>('delete_list', { id }),

  getTasks: (listId: number) => invoke<Task[]>('get_tasks', { listId }),
  createTask: (listId: number, title: string, priority?: string, dueDate?: string, description?: string) =>
    invoke<Task>('create_task', { listId, title, priority, dueDate, description }),
  updateTask: (
    id: number,
    fields: { title?: string; description?: string; priority?: string; dueDate?: string; completed?: boolean; position?: number; status?: string }
  ) => invoke<Task>('update_task', { id, ...fields }),
  deleteTask: (id: number) => invoke<void>('delete_task', { id }),
  createSubtask: (parentTaskId: number, title: string) =>
    invoke<Task>('create_subtask', { parentTaskId, title }),
  getSubtasks: (taskId: number) =>
    invoke<Task[]>('get_subtasks', { taskId }),
  createChildTask: (parentTaskId: number, title: string) =>
    invoke<Task>('create_child_task', { parentTaskId, title }),
  getChildTasks: (taskId: number) =>
    invoke<Task[]>('get_child_tasks', { taskId }),
  getDashboardTasks: () => invoke<DashboardData>('get_dashboard_tasks'),
  getProductivityStats: () => invoke<ProductivityStats>('get_productivity_stats'),
  setTaskTags: (taskId: number, tagNames: string[]) =>
    invoke<Tag[]>('set_task_tags', { taskId, tagNames }),
  getAllTags: () => invoke<Tag[]>('get_all_tags'),

  startTimer: (taskId: number) => invoke<StartTimerResult>('start_timer', { taskId }),
  stopTimer: (taskId: number) => invoke<void>('stop_timer', { taskId }),
  getActiveTimers: () => invoke<ActiveTimer[]>('get_active_timers'),
  getTimerSessions: (taskId: number) =>
    invoke<TimerSession[]>('get_timer_sessions', { taskId }),

  getSettings: () => invoke<Settings>('get_settings'),
  setSetting: (key: string, value: string) => invoke<void>('set_setting', { key, value }),

  getSyncStatus: () => invoke<SyncStatus>('get_sync_status'),
  setSyncFolder: (path: string) => invoke<SyncReport>('set_sync_folder', { path }),
  disableSync: () => invoke<void>('disable_sync'),
  syncNow: () => invoke<SyncReport>('sync_now'),
  setDeviceName: (name: string) => invoke<void>('set_device_name', { name }),

  getConflicts: () => invoke<SyncConflict[]>('get_conflicts'),
  resolveConflict: (id: number, restoreDiscarded: boolean) =>
    invoke<void>('resolve_conflict', { id, restoreDiscarded }),
}
