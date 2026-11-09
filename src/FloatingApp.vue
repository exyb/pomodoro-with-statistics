<script setup lang="ts">
import { computed } from 'vue'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { clock } from '@/lib/format'
import { PHASE_LABEL } from '@/lib/types'

const store = useAppStore()
const { snapshot, settings, prompts, running, paused, idle, error } = storeToRefs(store)

const phase = computed(() => snapshot.value?.phase ?? 'work')
const remaining = computed(() => snapshot.value?.remainingMs ?? 0)
const progress = computed(() => Math.round((snapshot.value?.progress ?? 0) * 100))
const showSeconds = computed(() => settings.value?.appearance.showSeconds ?? true)
const timeText = computed(() => (showSeconds.value ? clock(remaining.value) : clock(remaining.value).slice(0, 5)))

const floatSettings = computed(() => settings.value?.floating)
const showPrompt = computed(() => floatSettings.value?.showPrompts ?? true)
const prompt = computed(() => prompts.value.find((p) => p.slot === 'floating') ?? prompts.value.find((p) => p.slot === 'banner') ?? null)

const primaryLabel = computed(() => (running.value ? '暂停' : paused.value ? '继续' : '开始'))
const accentColor = computed(() => (phase.value === 'work' ? 'var(--accent)' : phase.value === 'shortBreak' ? 'var(--break)' : 'var(--long-break)'))
</script>

<template>
  <div class="floating" :class="{ 'click-through': floatSettings?.clickThrough, paused: paused }" :data-phase="phase">
    <div class="drag" data-tauri-drag-region />

    <div class="phase" :style="{ color: accentColor }">{{ PHASE_LABEL[phase] }}</div>
    <div class="time">{{ timeText }}</div>

    <div v-if="floatSettings?.showProgress" class="bar">
      <i :style="{ width: `${progress}%`, background: accentColor }" />
    </div>

    <div v-if="snapshot?.description" class="desc" :title="snapshot.description">{{ snapshot.description }}</div>

    <div v-if="showPrompt && prompt" class="prompt" :title="prompt.body">
      <b>{{ prompt.data?.words?.length ? prompt.data.words[0].en : prompt.title }}</b>
      <span>{{ prompt.data?.words?.length ? prompt.data.words[0].zh : prompt.body }}</span>
    </div>

    <div class="row">
      <button class="mini" :disabled="!store.ready" @click="store.toggleRun()">{{ primaryLabel }}</button>
      <button class="mini" :disabled="idle" title="停止并记录" @click="store.stop()">停止</button>
      <button class="mini" :disabled="idle" title="跳到下一阶段" @click="store.skip()">跳过</button>
      <button class="mini" title="打开主窗口" @click="store.showMain()">主窗</button>
      <button class="mini" title="关闭悬浮窗" @click="store.setFloating(false)">×</button>
    </div>

    <div v-if="error" class="error">{{ error }}</div>
    <div v-else-if="!store.ready" class="error">连接中…</div>
  </div>
</template>

<style scoped>
.floating {
  position: relative;
}

.floating .prompt {
  display: flex;
  flex-direction: column;
  line-height: 1.3;
}

.floating.paused .time {
  opacity: 0.55;
}

.floating .row {
  margin-top: 0.1rem;
}

.error {
  font-size: 0.65rem;
  color: var(--warn);
}
</style>
