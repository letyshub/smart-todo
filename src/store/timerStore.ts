import { create } from 'zustand'
import { api } from '../lib/tauri'

interface TimerStore {
  activeTaskId: number | null
  elapsedSeconds: number
  _intervalId: ReturnType<typeof setInterval> | null
  start: (taskId: number) => Promise<number | null>
  stop: (taskId: number) => Promise<void>
  _poll: () => Promise<void>
}

export const useTimerStore = create<TimerStore>((set, get) => ({
  activeTaskId: null,
  elapsedSeconds: 0,
  _intervalId: null,
  start: async (taskId) => {
    const result = await api.startTimer(taskId)
    const prev = get()._intervalId
    if (prev) clearInterval(prev)
    const id = setInterval(() => get()._poll(), 1000)
    set({ activeTaskId: taskId, elapsedSeconds: 0, _intervalId: id })
    return result.stopped_task_id
  },
  stop: async (taskId) => {
    await api.stopTimer(taskId)
    const id = get()._intervalId
    if (id) clearInterval(id)
    set({ activeTaskId: null, elapsedSeconds: 0, _intervalId: null })
  },
  _poll: async () => {
    const timers = await api.getActiveTimers()
    if (timers.length === 0) {
      const id = get()._intervalId
      if (id) clearInterval(id)
      set({ activeTaskId: null, elapsedSeconds: 0, _intervalId: null })
      return
    }
    set({ activeTaskId: timers[0].task_id, elapsedSeconds: timers[0].elapsed_seconds })
  },
}))
