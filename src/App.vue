<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, watch } from 'vue'
import { storeToRefs } from 'pinia'
import TaskSidebar from './components/TaskSidebar.vue'
import TimerPanel from './components/TimerPanel.vue'
import StatsView from './components/StatsView.vue'
import SettingsView from './components/SettingsView.vue'
import { useAppStore, type ViewName } from '@/stores/app'
import { clock } from '@/lib/format'
import { PHASE_LABEL } from '@/lib/types'

const store = useAppStore()
const { view, sidebarOpen, settings, snapshot, error, floatingOn } = storeToRefs(store)

const tabs: { id: ViewName; label: string }[] = [
  { id: 'timer', label: '计时' },
  { id: 'stats', label: '统计' },
  { id: 'settings', label: '设置' },
]

/** Resolved light/dark, so the toggle shows what the screen actually looks like. */
const themeMode = computed(() => {
  const theme = settings.value?.appearance.theme ?? 'system'
  if (theme !== 'system') return theme
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
})
const themeLabel = computed(() => (themeMode.value === 'dark' ? '☀' : '☾'))

function toggleTheme() {
  void store.patch((draft) => {
    draft.appearance.theme = themeMode.value === 'dark' ? 'light' : 'dark'
  })
}

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  if (!el) return false
  const tag = el.tagName
  return tag === 'INPUT' || tag === 'TEXTAREA' || el.isContentEditable
}

function onKeydown(event: KeyboardEvent) {
  if (isTyping(event.target)) return
  const ctrl = event.ctrlKey || event.metaKey
  if (event.code === 'Space' && !ctrl) {
    event.preventDefault()
    void store.toggleRun()
    return
  }
  if (!ctrl) return
  const key = event.key.toLowerCase()
  if (key === 's') {
    event.preventDefault()
    void store.stop()
  } else if (key === 'k') {
    event.preventDefault()
    void store.skip()
  } else if (key === 'b') {
    event.preventDefault()
    store.toggleSidebar()
  } else if (key === 'f') {
    event.preventDefault()
    void store.toggleFloating()
  } else if (key === '1' || key === '2' || key === '3') {
    event.preventDefault()
    store.setView(tabs[Number(key) - 1].id)
  }
}

watch(
  snapshot,
  (snap) => {
    if (!snap || snap.status === 'idle') {
      document.title = '番茄钟'
      return
    }
    const time = clock(snap.remainingMs)
    document.title = snap.status === 'paused' ? `⏸ ${time} · ${PHASE_LABEL[snap.phase]}` : `${time} · ${PHASE_LABEL[snap.phase]}`
  },
  { immediate: true },
)

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="app">
    <header class="topbar">
      <div class="brand">
        <i class="dot" />
        <span>番茄钟</span>
      </div>
      <nav class="tabs">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="tab"
          :class="{ active: view === tab.id }"
          @click="store.setView(tab.id)"
        >
          {{ tab.label }}
        </button>
      </nav>
      <div class="spacer" />
      <button
        class="btn ghost"
        :title="sidebarOpen ? '折叠待办侧栏 (Ctrl+B)' : '展开待办侧栏 (Ctrl+B)'"
        @click="store.toggleSidebar()"
      >
        待办
      </button>
      <button
        class="btn"
        :class="{ ghost: !floatingOn }"
        :title="floatingOn ? '关闭极简悬浮窗 (Ctrl+F)' : '打开极简悬浮窗 (Ctrl+F)'"
        @click="store.toggleFloating()"
      >
        悬浮窗
      </button>
      <button class="theme-btn" :title="`当前配色：${themeMode}，点击切换明暗`" @click="toggleTheme">
        {{ themeLabel }}
      </button>
    </header>

    <div class="shell">
      <aside class="sidebar" :class="{ collapsed: !sidebarOpen }">
        <div class="sidebar-inner">
          <TaskSidebar />
        </div>
      </aside>
      <main class="content">
        <div v-if="!store.ready" class="empty">正在连接本地计时引擎…</div>
        <template v-else>
          <TimerPanel v-if="view === 'timer'" />
          <StatsView v-else-if="view === 'stats'" />
          <SettingsView v-else />
        </template>
      </main>
    </div>

    <div v-if="error" class="toast">{{ error }}</div>
  </div>
</template>

<style scoped>
.theme-btn {
  border: none;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  font-size: 1rem;
  padding: 0.3rem 0.5rem;
  border-radius: 0.5rem;
}

.theme-btn:hover {
  color: var(--text);
  background: var(--panel-2);
}
</style>
