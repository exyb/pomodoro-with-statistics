import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type {
  Bootstrap,
  Phase,
  PluginMeta,
  Prompt,
  SessionFinished,
  SessionRow,
  Settings,
  Snapshot,
  StatsReport,
  Task,
} from './types'

export const api = {
  bootstrap: () => invoke<Bootstrap>('bootstrap'),
  start: (phase: Phase, minutes: number | null, description: string, taskId: number | null) =>
    invoke<Snapshot>('timer_start', { args: { phase, minutes, description, taskId } }),
  pause: () => invoke<Snapshot>('timer_pause'),
  resume: () => invoke<Snapshot>('timer_resume'),
  stop: () => invoke<Snapshot>('timer_stop'),
  skip: () => invoke<Snapshot>('timer_skip'),
  setDuration: (minutes: number) => invoke<Snapshot>('timer_set_duration', { args: { minutes } }),
  setContext: (description: string, taskId: number | null) =>
    invoke<Snapshot>('timer_set_context', { args: { description, taskId } }),
  saveSettings: (settings: Settings) => invoke<Settings>('settings_save', { settings }),
  testSound: (kind: Phase) => invoke<void>('settings_test_sound', { args: { kind } }),
  listTasks: () => invoke<Task[]>('tasks_list'),
  saveTask: (payload: {
    id: number | null
    title: string
    notes?: string
    plannedPomodoros?: number
    status?: string
  }) => invoke<Task>('tasks_save', { args: payload }),
  deleteTask: (id: number) => invoke<void>('tasks_delete', { args: { id } }),
  reorderTasks: (ids: number[]) => invoke<void>('tasks_reorder', { args: { ids } }),
  listSessions: (limit: number) => invoke<SessionRow[]>('sessions_list', { args: { limit } }),
  deleteSession: (id: number) => invoke<void>('session_delete', { args: { id } }),
  stats: (granularity: 'day' | 'week', count: number) =>
    invoke<StatsReport>('stats_query', { args: { granularity, count } }),
  plugins: () => invoke<PluginMeta[]>('plugins_list'),
  updatePlugin: (id: string, enabled: boolean | null, config: unknown) =>
    invoke<PluginMeta[]>('plugin_update', { args: { id, enabled, config } }),
  showFloating: () => invoke<void>('floating_show'),
  hideFloating: () => invoke<void>('floating_hide'),
  setClickThrough: (on: boolean) => invoke<void>('floating_click_through', { args: { on } }),
  showMain: () => invoke<void>('app_show_main'),
  quit: () => invoke<void>('app_quit'),
}

export const events = {
  onTick: (cb: (s: Snapshot) => void) => listen<Snapshot>('pomodoro://tick', (e) => cb(e.payload)),
  onFinished: (cb: (s: SessionFinished) => void) =>
    listen<SessionFinished>('pomodoro://session-finished', (e) => cb(e.payload)),
  onPrompt: (cb: (p: Prompt) => void) => listen<Prompt>('pomodoro://prompt', (e) => cb(e.payload)),
  onSettings: (cb: (s: Settings) => void) =>
    listen<Settings>('pomodoro://settings-changed', (e) => cb(e.payload)),
  onTasks: (cb: () => void) => listen('pomodoro://tasks-changed', () => cb()),
  onData: (cb: () => void) => listen('pomodoro://data-changed', () => cb()),
}
