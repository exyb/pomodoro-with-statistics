<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { api } from '@/lib/api'
import { applyFloatOpacity, applyScale, applyAccent, applyTheme } from '@/lib/theme'
import type { PluginMeta, Settings } from '@/lib/types'
import { useAppStore } from '@/stores/app'

const store = useAppStore()
const { settings, plugins, soundBackend } = storeToRefs(store)

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value) as string) as T
}

const local = ref<Settings>(clone(settings.value!))
const saved = ref(true)
let saveTimer: number | undefined

const newWorkPreset = ref<number | null>(null)
const newBreakPreset = ref<number | null>(null)

// Backend-driven changes (mini window closed, tray toggled a plugin) win over the form.
watch(settings, (incoming) => {
  if (!incoming) return
  if (JSON.stringify(incoming) !== JSON.stringify(local.value)) local.value = clone(incoming)
})

function queueSave() {
  saved.value = false
  window.clearTimeout(saveTimer)
  saveTimer = window.setTimeout(() => void flushSave(), 420)
}

/** The mini window is sized at creation, so these two knobs need a rebuild. */
function floatingSizeKey(s: Settings): string {
  return `${s.floating.scale}|${s.floating.showPrompts}`
}

async function flushSave() {
  window.clearTimeout(saveTimer)
  const before = settings.value
  await store.patch((draft) => {
    Object.assign(draft, clone(local.value))
  })
  saved.value = true
  const after = settings.value
  if (after?.floating.enabled && before && floatingSizeKey(before) !== floatingSizeKey(after)) {
    await store.rebuildFloating()
  }
}

/** Sliders repaint instantly; persistence still goes through the debounced save. */
function previewAppearance() {
  applyTheme(local.value.appearance.theme)
  applyScale(local.value.appearance.uiScale)
  applyAccent(local.value.appearance.accent)
  applyFloatOpacity(local.value.floating.opacity)
}

watch(local, () => {
  previewAppearance()
  queueSave()
}, { deep: true })

function resetScale() {
  local.value.appearance.uiScale = 1
}

function removePreset(kind: 'work' | 'break', index: number) {
  const list = kind === 'work' ? local.value.timers.workPresets : local.value.timers.breakPresets
  list.splice(index, 1)
}

function addPreset(kind: 'work' | 'break') {
  const minutes = kind === 'work' ? newWorkPreset.value : newBreakPreset.value
  if (!minutes || minutes < 1 || minutes > 180) {
    store.flash('请输入 1-180 之间的分钟数')
    return
  }
  const list = kind === 'work' ? local.value.timers.workPresets : local.value.timers.breakPresets
  if (list.includes(minutes)) {
    store.flash('这个时长已经在预设里了')
    return
  }
  list.push(minutes)
  list.sort((a, b) => a - b)
  if (kind === 'work') newWorkPreset.value = null
  else newBreakPreset.value = null
}

async function toggleClickThrough(on: boolean) {
  local.value.floating.clickThrough = on
  await api.setClickThrough(on)
}

function testSound(kind: 'work' | 'shortBreak') {
  void api.testSound(kind)
}

/* ---------- plugins ---------- */

function configOf(meta: PluginMeta): Record<string, unknown> {
  return { ...meta.config.default, ...(meta.config.current ?? {}) }
}

function setPluginConfig(meta: PluginMeta, key: string, value: unknown) {
  const next = configOf(meta)
  next[key] = value
  void store.setPlugin(meta.id, null, next)
}

/** Numbers come back as NaN from text inputs, so read the value by input type. */
function onConfigChange(meta: PluginMeta, key: string, event: Event) {
  const el = event.target as HTMLInputElement
  setPluginConfig(meta, key, el.type === 'number' ? el.valueAsNumber : el.value)
}

onBeforeUnmount(() => {
  window.clearTimeout(saveTimer)
  if (!saved.value) void flushSave()
})
</script>

<template>
  <div class="settings-grid">
    <section class="card">
      <h3>计时器</h3>
      <p class="hint">改动会自动保存，番茄钟按这里的长度计时。</p>
      <div class="grid-2">
        <label class="field">
          <span>专注（分钟）</span>
          <input v-model.number="local.timers.workMinutes" class="input num-input" type="number" min="1" max="180" />
        </label>
        <label class="field">
          <span>短休息（分钟）</span>
          <input v-model.number="local.timers.shortBreakMinutes" class="input num-input" type="number" min="1" max="60" />
        </label>
        <label class="field">
          <span>长休息（分钟）</span>
          <input v-model.number="local.timers.longBreakMinutes" class="input num-input" type="number" min="1" max="90" />
        </label>
        <label class="field">
          <span>每几个番茄后长休息</span>
          <input v-model.number="local.timers.longBreakInterval" class="input num-input" type="number" min="2" max="12" />
        </label>
      </div>

      <div class="field preset-block">
        <span>专注时长快捷键</span>
        <div class="preset-editor">
          <span v-for="(minutes, i) in local.timers.workPresets" :key="`w${minutes}`" class="chip static">
            {{ minutes }}
            <button class="icon-btn" title="移除" @click="removePreset('work', i)">×</button>
          </span>
          <input v-model.number="newWorkPreset" class="input num-input" type="number" min="1" max="180" placeholder="新增" />
          <button class="btn ghost" @click="addPreset('work')">添加</button>
        </div>
      </div>

      <div class="field preset-block">
        <span>休息时长快捷键</span>
        <div class="preset-editor">
          <span v-for="(minutes, i) in local.timers.breakPresets" :key="`b${minutes}`" class="chip static">
            {{ minutes }}
            <button class="icon-btn" title="移除" @click="removePreset('break', i)">×</button>
          </span>
          <input v-model.number="newBreakPreset" class="input num-input" type="number" min="1" max="180" placeholder="新增" />
          <button class="btn ghost" @click="addPreset('break')">添加</button>
        </div>
      </div>
    </section>

    <section class="card">
      <h3>外观</h3>
      <p class="hint">缩放会立刻生效，整块界面按字号等比放大。</p>

      <div class="set-row">
        <div>
          <div class="label">配色</div>
          <div class="desc">跟随系统会随 KDE 的深浅色切换</div>
        </div>
        <div class="row">
          <button
            v-for="option in (['system', 'light', 'dark'] as const)"
            :key="option"
            class="chip"
            :class="{ active: local.appearance.theme === option }"
            @click="local.appearance.theme = option"
          >
            {{ option === 'system' ? '跟随系统' : option === 'light' ? '浅色' : '深色' }}
          </button>
        </div>
      </div>

      <div class="set-row column">
        <div class="row between wide">
          <div class="label">界面缩放 · {{ Math.round(local.appearance.uiScale * 100) }}%</div>
          <button class="btn ghost" @click="resetScale">回到 100%</button>
        </div>
        <input v-model.number="local.appearance.uiScale" type="range" min="0.7" max="1.8" step="0.05" />
      </div>

      <div class="set-row">
        <div>
          <div class="label">强调色</div>
          <div class="desc">留空使用内置番茄红</div>
        </div>
        <div class="row">
          <input v-model="local.appearance.accent" class="color-input" type="color" />
          <button class="btn ghost" @click="local.appearance.accent = ''">重置</button>
        </div>
      </div>

      <div class="set-row">
        <div class="label">倒计时显示秒</div>
        <label class="switch">
          <input v-model="local.appearance.showSeconds" type="checkbox" />
          <i class="track" />
        </label>
      </div>

      <div class="set-row">
        <div class="label">紧凑布局</div>
        <label class="switch">
          <input v-model="local.appearance.compactLayout" type="checkbox" />
          <i class="track" />
        </label>
      </div>
    </section>

    <section class="card">
      <h3>提醒</h3>
      <p class="hint">
        声音后端：{{ soundBackend || '未检测到 pw-play/paplay/aplay' }}；提示音由本机播放器播放。
      </p>

      <div class="set-row">
        <div class="label">阶段结束时发声</div>
        <label class="switch">
          <input v-model="local.alerts.soundEnabled" type="checkbox" />
          <i class="track" />
        </label>
      </div>

      <div class="set-row column">
        <div class="row between wide">
          <div class="label">音量 · {{ local.alerts.volume }}</div>
          <div class="row">
            <button class="btn ghost" @click="testSound('work')">试听专注</button>
            <button class="btn ghost" @click="testSound('shortBreak')">试听休息</button>
          </div>
        </div>
        <input v-model.number="local.alerts.volume" type="range" min="0" max="100" step="5" />
      </div>

      <label class="field">
        <span>专注提示音文件（留空用内置；支持 wav/mp3/ogg 绝对路径）</span>
        <input v-model="local.alerts.workSoundPath" class="input" placeholder="/home/you/.local/share/…/work.wav" />
      </label>
      <label class="field">
        <span>休息提示音文件</span>
        <input v-model="local.alerts.breakSoundPath" class="input" placeholder="/home/you/Music/break.ogg" />
      </label>

      <div class="set-row">
        <div class="label">桌面通知（KDE 通知中心）</div>
        <label class="switch">
          <input v-model="local.alerts.desktopNotification" type="checkbox" />
          <i class="track" />
        </label>
      </div>

      <div class="set-row">
        <div class="label">系统托盘常驻</div>
        <label class="switch">
          <input v-model="local.alerts.trayEnabled" type="checkbox" />
          <i class="track" />
        </label>
      </div>
    </section>

    <section class="card">
      <h3>行为</h3>
      <div class="set-row">
        <div class="label">专注结束后自动开始休息</div>
        <label class="switch">
          <input v-model="local.behaviour.autoStartBreaks" type="checkbox" />
          <i class="track" />
        </label>
      </div>
      <div class="set-row">
        <div class="label">休息结束后自动开始下一个专注</div>
        <label class="switch">
          <input v-model="local.behaviour.autoStartWork" type="checkbox" />
          <i class="track" />
        </label>
      </div>
      <div class="set-row">
        <div>
          <div class="label">点关闭只隐藏到托盘</div>
          <div class="desc">关掉后从托盘菜单可以再次打开</div>
        </div>
        <label class="switch">
          <input v-model="local.behaviour.closeToTray" type="checkbox" />
          <i class="track" />
        </label>
      </div>
      <div class="set-row">
        <div>
          <div class="label">手动开始时重置循环计数</div>
          <div class="desc">影响长休息的节奏</div>
        </div>
        <label class="switch">
          <input v-model="local.behaviour.resetCycleOnManualStart" type="checkbox" />
          <i class="track" />
        </label>
      </div>
      <div class="set-row">
        <div>
          <div class="label">开机自启</div>
          <div class="desc">写入 KDE autostart 目录</div>
        </div>
        <label class="switch">
          <input v-model="local.behaviour.startAtLogin" type="checkbox" />
          <i class="track" />
        </label>
      </div>
    </section>

    <section class="card">
      <h3>极简悬浮窗</h3>
      <p class="hint">独立置顶小窗，不参与任务栏，可拖动位置。</p>

      <div class="set-row">
        <div class="label">显示悬浮窗</div>
        <label class="switch">
          <input v-model="local.floating.enabled" type="checkbox" />
          <i class="track" />
        </label>
      </div>

      <div class="set-row column">
        <div class="label">背景不透明度 · {{ Math.round(local.floating.opacity * 100) }}%</div>
        <input v-model.number="local.floating.opacity" type="range" min="0.3" max="1" step="0.02" />
      </div>

      <div class="set-row column">
        <div class="label">窗口大小 · {{ Math.round(local.floating.scale * 100) }}%</div>
        <input v-model.number="local.floating.scale" type="range" min="0.7" max="1.6" step="0.05" />
      </div>

      <div class="set-row">
        <div>
          <div class="label">进度条</div>
          <div class="desc">关闭后只显示时间与阶段</div>
        </div>
        <label class="switch">
          <input v-model="local.floating.showProgress" type="checkbox" />
          <i class="track" />
        </label>
      </div>

      <div class="set-row">
        <div>
          <div class="label">显示扩展提示</div>
          <div class="desc">单词卡等提示会占一行，窗口会更高</div>
        </div>
        <label class="switch">
          <input v-model="local.floating.showPrompts" type="checkbox" />
          <i class="track" />
        </label>
      </div>

      <div class="set-row">
        <div>
          <div class="label">点击穿透</div>
          <div class="desc">鼠标可以穿过小窗，需要用托盘或这里关闭</div>
        </div>
        <label class="switch">
          <input :checked="local.floating.clickThrough" type="checkbox" @change="toggleClickThrough(($event.target as HTMLInputElement).checked)" />
          <i class="track" />
        </label>
      </div>
    </section>

    <section class="card">
      <h3>扩展插件</h3>
      <p class="hint">
        插件由 Rust 侧产生提示，前端按 kind 渲染。接口见 docs/PLUGIN-API.md。
      </p>
      <p v-if="!plugins.length" class="empty">没有已注册的插件。</p>
      <div v-for="meta in plugins" :key="meta.id" class="plugin">
        <div class="set-row">
          <div>
            <div class="label">{{ meta.name }}</div>
            <div class="desc">{{ meta.description }}</div>
          </div>
          <label class="switch">
            <input :checked="meta.enabled" type="checkbox" @change="store.setPlugin(meta.id, ($event.target as HTMLInputElement).checked, null)" />
            <i class="track" />
          </label>
        </div>
        <div v-if="meta.enabled" class="plugin-config">
          <div v-for="[key, value] in Object.entries(configOf(meta))" :key="key" class="set-row">
            <div class="label">{{ key }}</div>
            <label v-if="typeof value === 'boolean'" class="switch">
              <input :checked="value" type="checkbox" @change="setPluginConfig(meta, key, ($event.target as HTMLInputElement).checked)" />
              <i class="track" />
            </label>
            <input
              v-else
              class="input num-input"
              :class="{ wide: typeof value === 'string' }"
              :type="typeof value === 'number' ? 'number' : 'text'"
              :value="value"
              @change="onConfigChange(meta, key, $event)"
            />
          </div>
        </div>
      </div>
      <div class="save-state">{{ saved ? '设置已保存' : '保存中…' }}</div>
    </section>
  </div>
</template>

<style scoped>
.preset-block {
  margin-top: 0.7rem;
  gap: 0.4rem;
}

.preset-block > span {
  font-size: 0.8rem;
  color: var(--text-dim);
}

.set-row.column {
  flex-direction: column;
  align-items: stretch;
  gap: 0.35rem;
}

.row.between.wide,
.row.between {
  width: 100%;
  justify-content: space-between;
}

.color-input {
  width: 2.6rem;
  height: 1.8rem;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 0.5rem;
  background: var(--panel);
}

.plugin {
  border-top: 1px solid var(--border);
  padding-top: 0.4rem;
  margin-top: 0.4rem;
}

.plugin-config {
  padding-left: 0.5rem;
}

.num-input.wide {
  width: 12rem;
}

.save-state {
  margin-top: 0.6rem;
  font-size: 0.74rem;
  color: var(--text-dim);
}
</style>
