<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import RingProgress from './RingProgress.vue'
import PromptSlot from './prompts/PromptSlot.vue'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { clock, humanDuration } from '@/lib/format'
import { PHASE_LABEL, type Task } from '@/lib/types'

const store = useAppStore()
const { snapshot, settings, today, boundTask, running, paused, idle, phase, progress, cycleIndex, activePresets, longBreakInterval } =
  storeToRefs(store)

const draft = ref(snapshot.value?.description ?? '')
const customMinutes = ref<number | null>(null)
const dropActive = ref(false)
let syncTimer: number | undefined

const showSeconds = computed(() => settings.value?.appearance.showSeconds ?? true)
const remaining = computed(() => snapshot.value?.remainingMs ?? 0)
const timeText = computed(() => (showSeconds.value ? clock(remaining.value) : clock(remaining.value).slice(0, 5)))
const ringColor = computed(() => (phase.value === 'work' ? 'var(--accent)' : 'var(--break)'))

const todayBucket = computed(() => today.value?.buckets[today.value.buckets.length - 1])
const cycleDots = computed(() => {
  const total = longBreakInterval.value ?? 4
  const done = ((cycleIndex.value - 1) % total + total) % total
  return Array.from({ length: total }, (_, i) => i < (cycleIndex.value % total === 0 && cycleIndex.value > 0 ? total : done))
})

const nextHint = computed(() => {
  if (!snapshot.value || !settings.value) return ''
  if (!idle.value) {
    return paused.value ? '已暂停，暂停时间计入中断时长' : `已用 ${humanDuration((snapshot.value.elapsedMs ?? 0) / 1000)}`
  }
  const auto = phase.value === 'work' ? settings.value.behaviour.autoStartWork : settings.value.behaviour.autoStartBreaks
  return auto ? '上一阶段结束后会自动开始' : '上一阶段结束，等待你开始'
})

watch(
  () => snapshot.value?.description,
  (value) => {
    if (value == null) return
    // Never overwrite what is being typed.
    if (document.activeElement?.classList.contains('desc-input')) return
    draft.value = value
  },
  { immediate: true },
)

function queueCommit() {
  window.clearTimeout(syncTimer)
  syncTimer = window.setTimeout(() => {
    if (draft.value !== (snapshot.value?.description ?? '')) void store.setDescription(draft.value)
  }, 450)
}

function commitNow() {
  window.clearTimeout(syncTimer)
  if (draft.value !== (snapshot.value?.description ?? '')) void store.setDescription(draft.value)
}

function pickPreset(minutes: number) {
  void store.chooseDuration(minutes)
  if (idle.value) void store.start({ minutes })
}

function applyCustom() {
  const minutes = customMinutes.value
  if (!minutes || minutes < 1 || minutes > 180) {
    store.flash('请输入 1-180 分钟的自定义时长')
    return
  }
  pickPreset(minutes)
}

/** Drop a todo from the sidebar onto the description area. */
function readTask(event: DragEvent): Task | null {
  const raw = event.dataTransfer?.getData('application/x-pomodoro-task') || event.dataTransfer?.getData('text/plain') || ''
  const id = Number(raw)
  if (!Number.isFinite(id) || id <= 0) return null
  return store.tasks.find((t) => t.id === id) ?? null
}

function onDrop(event: DragEvent) {
  dropActive.value = false
  const task = readTask(event)
  if (!task) return
  draft.value = task.title
  void store.bindTask(task)
  void store.loadToday()
}

function onDragOver(event: DragEvent) {
  dropActive.value = true
  event.preventDefault()
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy'
}

function onDragLeave() {
  dropActive.value = false
}

function primaryAction() {
  void store.toggleRun()
}

const primaryLabel = computed(() => (running.value ? '暂停' : paused.value ? '继续' : '开始'))

function useDraftAsStart() {
  commitNow()
  if (idle.value) void store.start({ description: draft.value })
}

defineExpose({ useDraftAsStart })
</script>

<template>
  <div class="timer-panel">
    <div class="ring-wrap">
      <RingProgress :progress="progress" :color="ringColor" />
      <div class="ring-center">
        <div class="ring-phase">{{ PHASE_LABEL[phase] }}</div>
        <div class="ring-time">{{ timeText }}</div>
        <div class="cycle-dots">
          <i v-for="(on, i) in cycleDots" :key="i" :class="{ on }" />
        </div>
        <div v-if="nextHint" class="hint">{{ nextHint }}</div>
      </div>
    </div>

    <div v-if="settings" class="presets row">
      <button
        v-for="minutes in activePresets"
        :key="minutes"
        class="chip"
        :class="{ active: Math.round((snapshot?.plannedMs ?? 0) / 60000) === minutes }"
        :title="idle ? `开始 ${minutes} 分钟` : `把本阶段改为 ${minutes} 分钟`"
        @click="pickPreset(minutes)"
      >
        {{ minutes }} 分钟
      </button>
      <div class="custom">
        <input v-model.number="customMinutes" class="input num-input" type="number" min="1" max="180" placeholder="自定义" />
        <button class="btn ghost" @click="applyCustom">应用</button>
      </div>
    </div>

    <div
      class="dropzone"
      :class="{ over: dropActive, bound: !!boundTask }"
      @drop="onDrop"
      @dragover="onDragOver"
      @dragleave="onDragLeave"
    >
      <div class="row between">
        <span class="section-title" style="margin: 0">本次做什么</span>
        <span v-if="boundTask" class="chip static active">
          关联待办：{{ boundTask.title }}
          <button class="icon-btn" title="取消关联" @click="store.unbindTask">×</button>
        </span>
        <span v-else class="hint-inline">把左侧待办拖进来，或直接输入</span>
      </div>
      <textarea
        v-model="draft"
        class="desc-input"
        rows="2"
        placeholder="例如：写完周报的数据部分 / 修复登录白屏"
        @input="queueCommit"
        @blur="commitNow"
      />
    </div>

    <div class="controls">
      <button class="btn primary big" :disabled="store.busy" @click="primaryAction">{{ primaryLabel }}</button>
      <button class="btn" :disabled="idle" @click="store.stop()">停止</button>
      <button class="btn" :disabled="idle" @click="store.skip()">跳过</button>
      <button v-if="idle" class="btn ghost" @click="useDraftAsStart" title="用当前描述开始一个专注计时">开始并记录</button>
    </div>

    <PromptSlot slot="timer" />

    <div v-if="todayBucket" class="mini-stats">
      <div class="cell">
        <b>{{ Math.round(todayBucket.focusSeconds / 60) }}</b><span>今日专注（分钟）</span>
      </div>
      <div class="cell"><b>{{ todayBucket.pomodoros }}</b><span>今日番茄</span></div>
      <div class="cell"><b>{{ todayBucket.interruptions }}</b><span>今日中断次数</span></div>
      <div class="cell">
        <b>{{ Math.round(todayBucket.interruptedSeconds / 60) }}</b><span>今日中断（分钟）</span>
      </div>
      <div class="cell"><b>{{ todayBucket.tasks }}</b><span>今日任务数</span></div>
    </div>

    <div class="kbd-hint">
      <kbd>空格</kbd> 开始/暂停 · <kbd>Ctrl</kbd>+<kbd>S</kbd> 停止 · <kbd>Ctrl</kbd>+<kbd>K</kbd> 跳过 ·
      <kbd>Ctrl</kbd>+<kbd>B</kbd> 折叠待办
    </div>
  </div>
</template>

<style scoped>
.row.between {
  justify-content: space-between;
}

.presets {
  justify-content: center;
}

.custom {
  display: flex;
  gap: 0.3rem;
  align-items: center;
}

.hint {
  font-size: 0.72rem;
  color: var(--text-dim);
  max-width: 12rem;
  text-align: center;
}

.hint-inline {
  font-size: 0.74rem;
  color: var(--text-dim);
}

.dropzone .row {
  margin-bottom: 0.3rem;
}

.kbd-hint {
  font-size: 0.72rem;
  color: var(--text-dim);
  text-align: center;
}
</style>
