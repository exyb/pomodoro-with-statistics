import { defineStore } from 'pinia'
import { api, events } from '@/lib/api'
import type { Phase, PluginMeta, Prompt, Settings, Snapshot, StatsReport, Task } from '@/lib/types'

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value) as string) as T
}

export type ViewName = 'timer' | 'stats' | 'settings'

interface State {
  ready: boolean
  settings: Settings | null
  snapshot: Snapshot | null
  tasks: Task[]
  plugins: PluginMeta[]
  soundBackend: string | null
  prompts: Prompt[]
  view: ViewName
  sidebarOpen: boolean
  floatingOn: boolean
  today: StatsReport | null
  error: string | null
  busy: boolean
}

export const useAppStore = defineStore('app', {
  state: (): State => ({
    ready: false,
    settings: null,
    snapshot: null,
    tasks: [],
    plugins: [],
    soundBackend: null,
    prompts: [],
    view: 'timer',
    sidebarOpen: true,
    floatingOn: false,
    today: null,
    error: null,
    busy: false,
  }),

  getters: {
    running: (s) => s.snapshot?.status === 'running',
    paused: (s) => s.snapshot?.status === 'paused',
    idle: (s) => !s.snapshot || s.snapshot.status === 'idle',
    phase: (s) => s.snapshot?.phase ?? 'work',
    remainingMs: (s) => s.snapshot?.remainingMs ?? 0,
    progress: (s) => s.snapshot?.progress ?? 0,
    cycleIndex: (s) => s.snapshot?.cycleIndex ?? 0,
    draftDescription: (s) => s.snapshot?.description ?? '',
    boundTask: (s) => {
      const id = s.snapshot?.taskId ?? null
      return id == null ? null : s.tasks.find((t) => t.id === id) ?? null
    },
    activePresets: (s) => {
      const t = s.settings?.timers
      if (!t) return [15, 20, 25, 30, 45]
      return (s.snapshot?.phase ?? 'work') === 'work' ? t.workPresets : t.breakPresets
    },
    longBreakInterval: (s) => s.settings?.timers.longBreakInterval ?? 4,
  },

  actions: {
    async init() {
      const boot = await api.bootstrap()
      this.settings = boot.settings
      this.snapshot = boot.snapshot
      this.tasks = boot.tasks
      this.plugins = boot.plugins
      this.soundBackend = boot.soundBackend
      this.floatingOn = boot.floatingVisible
      this.ready = true

      await this.loadToday()

      await events.onTick((payload: Snapshot) => {
        this.snapshot = payload
      })
      await events.onSettings((payload: Settings) => {
        this.settings = payload
        this.floatingOn = payload.floating.enabled
      })
      await events.onPrompt((payload: Prompt) => this.pushPrompt(payload))
      await events.onTasks(() => this.refreshTasks())
      await events.onData(() => {
        void this.loadToday()
      })
      await events.onFinished(() => {
        void this.refreshTasks()
        void this.loadToday()
      })
    },

    async loadToday() {
      this.today = await api.stats('day', 1)
    },

    async refreshTasks() {
      this.tasks = await api.listTasks()
    },

    setView(view: ViewName) {
      this.view = view
    },

    toggleSidebar() {
      this.sidebarOpen = !this.sidebarOpen
    },

    flash(message: string | null) {
      this.error = message
      if (message) setTimeout(() => (this.error = null), 2600)
    },

    pushPrompt(prompt: Prompt) {
      // One live prompt per slot: a newer card replaces the older one.
      this.prompts = this.prompts.filter((p) => p.slot !== prompt.slot)
      this.prompts.push(prompt)
      if (prompt.ttlMs > 0) {
        setTimeout(() => {
          this.prompts = this.prompts.filter((p) => p.id !== prompt.id)
        }, prompt.ttlMs)
      }
    },

    dismissPrompt(id: number) {
      this.prompts = this.prompts.filter((p) => p.id !== id)
    },

    /** Mutate a copy of the settings and persist it; the backend is the source of truth. */
    async patch(mutator: (draft: Settings) => void) {
      if (!this.settings) return
      const draft = clone(this.settings)
      mutator(draft)
      this.busy = true
      try {
        this.settings = await api.saveSettings(draft)
      } catch (e) {
        this.flash(String(e))
      } finally {
        this.busy = false
      }
    },

    async start(opts: { phase?: Phase; minutes?: number | null; description?: string; taskId?: number | null } = {}) {
      const snapshot = this.snapshot
      const phase = opts.phase ?? snapshot?.phase ?? 'work'
      const minutes = opts.minutes ?? null
      const description = opts.description ?? snapshot?.description ?? ''
      const taskId = opts.taskId ?? snapshot?.taskId ?? null
      this.busy = true
      try {
        this.snapshot = await api.start(phase, minutes, description, taskId)
        await this.loadToday()
      } catch (e) {
        this.flash(String(e))
      } finally {
        this.busy = false
      }
    },

    async toggleRun() {
      try {
        if (this.snapshot?.status === 'running') this.snapshot = await api.pause()
        else if (this.snapshot?.status === 'paused') this.snapshot = await api.resume()
        else await this.start()
      } catch (e) {
        this.flash(String(e))
      }
    },

    async stop() {
      try {
        this.snapshot = await api.stop()
        await Promise.all([this.loadToday(), this.refreshTasks()])
      } catch (e) {
        this.flash(String(e))
      }
    },

    async skip() {
      try {
        this.snapshot = await api.skip()
        await Promise.all([this.loadToday(), this.refreshTasks()])
      } catch (e) {
        this.flash(String(e))
      }
    },

    async chooseDuration(minutes: number) {
      this.snapshot = await api.setDuration(minutes)
    },

    async setDescription(description: string) {
      const taskId = this.snapshot?.taskId ?? null
      this.snapshot = await api.setContext(description, taskId)
    },

    /** Dragging (or clicking) a todo binds it and fills the description if it is empty. */
    async bindTask(task: Task) {
      this.snapshot = await api.setContext(this.snapshot?.description || task.title, task.id)
      if (task.status !== 'doing') {
        try {
          const updated = await api.saveTask({
            id: task.id,
            title: task.title,
            notes: task.notes,
            plannedPomodoros: task.plannedPomodoros,
            status: 'doing',
          })
          this.tasks = this.tasks.map((t) => (t.id === updated.id ? updated : t))
        } catch {
          /* binding is still useful even if the status flip fails */
        }
      }
    },

    async unbindTask() {
      this.snapshot = await api.setContext(this.snapshot?.description ?? '', null)
    },

    async addTask(title: string, notes = '') {
      const trimmed = title.trim()
      if (!trimmed) return
      const task = await api.saveTask({ id: null, title: trimmed, notes })
      this.tasks = [...this.tasks, task]
    },

    async updateTask(task: Task, patch: { title?: string; notes?: string; plannedPomodoros?: number; status?: string }) {
      const updated = await api.saveTask({
        id: task.id,
        title: patch.title ?? task.title,
        notes: patch.notes ?? task.notes,
        plannedPomodoros: patch.plannedPomodoros ?? task.plannedPomodoros,
        status: patch.status ?? task.status,
      })
      this.tasks = this.tasks.map((t) => (t.id === updated.id ? updated : t))
    },

    async deleteTask(task: Task) {
      await api.deleteTask(task.id)
      this.tasks = this.tasks.filter((t) => t.id !== task.id)
      if (this.snapshot?.taskId === task.id) this.snapshot = await api.setContext(this.snapshot.description, null)
    },

    async reorderTasks(ids: number[]) {
      this.tasks = ids.map((id) => this.tasks.find((t) => t.id === id)!).filter(Boolean)
      await api.reorderTasks(ids)
    },

    async setPlugin(id: string, enabled: boolean | null, config: unknown) {
      this.plugins = await api.updatePlugin(id, enabled, config)
    },

    async setFloating(on: boolean) {
      await this.patch((s) => {
        s.floating.enabled = on
      })
      if (on) await api.showFloating()
      this.floatingOn = on
    },

    async toggleFloating() {
      await this.setFloating(!this.floatingOn)
    },

    /** Used by the mini window to bring the main window back. */
    async showMain() {
      await api.showMain()
    },

    /** The mini window sizes itself at creation time, so a size-related change needs a rebuild. */
    async rebuildFloating() {
      if (!this.floatingOn) return
      await api.hideFloating()
      await api.showFloating()
      await this.patch((s) => {
        s.floating.enabled = true
      })
    },
  },
})
