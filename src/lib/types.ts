export type Phase = 'work' | 'shortBreak' | 'longBreak'
export type RunStatus = 'idle' | 'running' | 'paused'

export interface Snapshot {
  phase: Phase
  status: RunStatus
  plannedMs: number
  elapsedMs: number
  remainingMs: number
  pausedMs: number
  interruptions: number
  cycleIndex: number
  description: string
  taskId: number | null
  sessionId: number | null
  startedAtMs: number | null
  progress: number
}

export interface Task {
  id: number
  title: string
  notes: string
  plannedPomodoros: number
  completedPomodoros: number
  status: 'todo' | 'doing' | 'done'
  position: number
  createdAt: number
  updatedAt: number
  completedAt: number | null
}

export interface SessionRow {
  id: number
  kind: Phase
  taskId: number | null
  description: string
  plannedSeconds: number
  focusSeconds: number
  pausedSeconds: number
  interruptions: number
  startedAt: number
  endedAt: number | null
  status: 'running' | 'completed' | 'aborted'
  cycleIndex: number
}

export interface PluginSettings {
  enabled: boolean
  config: unknown
}

export interface Settings {
  timers: {
    workMinutes: number
    shortBreakMinutes: number
    longBreakMinutes: number
    longBreakInterval: number
    workPresets: number[]
    breakPresets: number[]
  }
  behaviour: {
    autoStartBreaks: boolean
    autoStartWork: boolean
    closeToTray: boolean
    resetCycleOnManualStart: boolean
    startAtLogin: boolean
  }
  appearance: {
    theme: 'light' | 'dark' | 'system'
    uiScale: number
    accent: string
    showSeconds: boolean
    compactLayout: boolean
  }
  alerts: {
    soundEnabled: boolean
    volume: number
    workSoundPath: string
    breakSoundPath: string
    desktopNotification: boolean
    trayEnabled: boolean
  }
  floating: {
    enabled: boolean
    opacity: number
    clickThrough: boolean
    scale: number
    showProgress: boolean
    showPrompts: boolean
    position: [number, number] | null
  }
  plugins: Record<string, PluginSettings>
}

export interface PluginMeta {
  id: string
  name: string
  description: string
  enabled: boolean
  config: { default: Record<string, unknown>; current: Record<string, unknown> | null }
  defaultConfig: Record<string, unknown>
}

export type PromptSlot = 'timer' | 'floating' | 'banner'

export interface Prompt {
  id: number
  pluginId: string
  slot: PromptSlot
  kind: string
  title: string
  body: string
  ttlMs: number
  data: { words?: { en: string; zh: string; example?: string }[]; seen?: number } | null
}

export interface SessionFinished {
  phase: Phase
  status: 'completed' | 'aborted'
  focusSeconds: number
  interruptedSeconds: number
  interruptions: number
  description: string
  taskId: number | null
  sessionId: number | null
  cycleIndex: number
}

export interface Bucket {
  key: string
  label: string
  subLabel: string
  startMs: number
  endMs: number
  focusSeconds: number
  pomodoros: number
  completed: number
  aborted: number
  interruptions: number
  interruptedSeconds: number
  breakSeconds: number
  tasks: number
}

export interface Summary {
  focusSeconds: number
  pomodoros: number
  completed: number
  aborted: number
  interruptions: number
  interruptedSeconds: number
  breakSeconds: number
  avgFocusSeconds: number
  bestBucketLabel: string
  bestFocusSeconds: number
  distinctTasks: number
  completionRate: number
  avgSessionMinutes: number
}

export interface TaskAgg {
  taskId: number | null
  title: string
  pomodoros: number
  focusSeconds: number
  interruptions: number
}

export interface StatsReport {
  granularity: 'day' | 'week'
  buckets: Bucket[]
  summary: Summary
  tasks: TaskAgg[]
  hourHeat: number[]
  rangeStartMs: number
  rangeEndMs: number
}

export interface Bootstrap {
  settings: Settings
  snapshot: Snapshot
  tasks: Task[]
  plugins: PluginMeta[]
  soundBackend: string | null
  floatingVisible: boolean
  platform: string
}

export const PHASE_LABEL: Record<Phase, string> = {
  work: '专注',
  shortBreak: '短休息',
  longBreak: '长休息',
}
