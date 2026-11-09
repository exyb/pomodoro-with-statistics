<script setup lang="ts">
import { computed, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import type { Task } from '@/lib/types'

const store = useAppStore()
const { tasks, snapshot } = storeToRefs(store)

type Filter = 'all' | 'open' | 'doing' | 'done'
const filter = ref<Filter>('all')
const draftTitle = ref('')
const editingId = ref<number | null>(null)
const editTitle = ref('')
const editNotes = ref('')
const editPlanned = ref(0)
const dragId = ref<number | null>(null)
const overId = ref<number | null>(null)

const filters: { id: Filter; label: string }[] = [
  { id: 'all', label: '全部' },
  { id: 'open', label: '待办' },
  { id: 'doing', label: '进行中' },
  { id: 'done', label: '已完成' },
]

const visible = computed(() => {
  const sorted = [...tasks.value].sort((a, b) => a.position - b.position)
  switch (filter.value) {
    case 'open':
      return sorted.filter((t) => t.status === 'todo')
    case 'doing':
      return sorted.filter((t) => t.status === 'doing')
    case 'done':
      return sorted.filter((t) => t.status === 'done')
    default:
      return sorted
  }
})

const openCount = computed(() => tasks.value.filter((t) => t.status !== 'done').length)

function submitNew() {
  const title = draftTitle.value.trim()
  if (!title) return
  void store.addTask(title)
  draftTitle.value = ''
}

function startEdit(task: Task) {
  editingId.value = task.id
  editTitle.value = task.title
  editNotes.value = task.notes
  editPlanned.value = task.plannedPomodoros
}

function saveEdit(task: Task) {
  const title = editTitle.value.trim()
  if (!title) {
    store.flash('标题不能为空')
    return
  }
  void store.updateTask(task, {
    title,
    notes: editNotes.value.trim(),
    plannedPomodoros: editPlanned.value,
  })
  editingId.value = null
}

function cycleStatus(task: Task) {
  const next = task.status === 'done' ? 'todo' : task.status === 'todo' ? 'doing' : 'done'
  void store.updateTask(task, { status: next })
}

/** Clicking a todo binds it to the timer, same as dragging it in. */
function pick(task: Task) {
  if (editingId.value === task.id) return
  void store.bindTask(task)
}

function onDragStart(event: DragEvent, task: Task) {
  dragId.value = task.id
  event.dataTransfer?.clearData()
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = 'copyMove'
    event.dataTransfer.setData('application/x-pomodoro-task', String(task.id))
    // Browsers/webviews that ignore custom types still deliver the title.
    event.dataTransfer.setData('text/plain', String(task.id))
  }
}

function onDragEnd() {
  dragId.value = null
  overId.value = null
}

function onDragOver(event: DragEvent, task: Task) {
  if (dragId.value == null || dragId.value === task.id) return
  overId.value = task.id
  event.preventDefault()
  if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
}

function onDrop(event: DragEvent, task: Task) {
  const source = dragId.value
  overId.value = null
  dragId.value = null
  event.preventDefault()
  event.stopPropagation()
  if (source == null || source === task.id) return
  const order = tasks.value.slice().sort((a, b) => a.position - b.position).map((t) => t.id)
  const from = order.indexOf(source)
  if (from < 0) return
  order.splice(from, 1)
  order.splice(order.indexOf(task.id), 0, source)
  void store.reorderTasks(order)
}

const boundId = computed(() => snapshot.value?.taskId ?? null)
</script>

<template>
  <section class="sidebar-inner-root">
    <div class="row between head">
      <h2 class="section-title">待办 · {{ openCount }}</h2>
      <span class="hint">拖到计时器即可关联</span>
    </div>

    <form class="add" @submit.prevent="submitNew">
      <input v-model="draftTitle" class="input" placeholder="新增一条待办，回车保存" />
    </form>

    <div class="filters row">
      <button
        v-for="f in filters"
        :key="f.id"
        class="chip"
        :class="{ active: filter === f.id }"
        @click="filter = f.id"
      >
        {{ f.label }}
      </button>
    </div>

    <p v-if="!visible.length" class="empty">这个筛选下还没有待办。</p>

    <ul v-else class="list">
      <li
        v-for="task in visible"
        :key="task.id"
        class="task-item"
        :class="{
          done: task.status === 'done',
          dragging: dragId === task.id,
          'drop-target': overId === task.id,
          bound: boundId === task.id,
        }"
        draggable="true"
        @dragstart="onDragStart($event, task)"
        @dragend="onDragEnd"
        @dragover="onDragOver($event, task)"
        @drop="onDrop($event, task)"
        @click="pick(task)"
      >
        <span
          class="mark"
          :class="{ on: task.status !== 'todo' }"
          :title="task.status === 'done' ? '点击重新打开' : task.status === 'doing' ? '标记完成' : '标记为进行中'"
          @click.stop="cycleStatus(task)"
        >
          {{ task.status === 'done' ? '✓' : task.status === 'doing' ? '▶' : '' }}
        </span>

        <div v-if="editingId === task.id" class="body edit" @click.stop>
          <input v-model="editTitle" class="input" placeholder="标题" @keydown.enter="saveEdit(task)" />
          <input v-model="editNotes" class="input" placeholder="备注（可选）" />
          <div class="row">
            <label class="field-label">计划番茄</label>
            <input v-model.number="editPlanned" class="input num-input" type="number" min="0" max="40" />
            <button class="btn" @click="saveEdit(task)">保存</button>
            <button class="btn ghost" @click="editingId = null">取消</button>
          </div>
        </div>

        <div v-else class="body">
          <div class="title">{{ task.title }}</div>
          <div v-if="task.notes" class="notes">{{ task.notes }}</div>
          <div class="sub">
            <span>{{ task.completedPomodoros }}<template v-if="task.plannedPomodoros">/{{ task.plannedPomodoros }}</template> 番茄</span>
            <span v-if="task.status === 'doing'">进行中</span>
            <span v-if="boundId === task.id" class="tag">已关联</span>
          </div>
        </div>

        <div class="actions">
          <button class="icon-btn" title="编辑" @click.stop="startEdit(task)">✎</button>
          <button class="icon-btn" title="删除" @click.stop="store.deleteTask(task)">×</button>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.sidebar-inner-root {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.head {
  justify-content: space-between;
  align-items: baseline;
}

.head .section-title {
  margin: 0;
}

.head .hint {
  font-size: 0.7rem;
  color: var(--text-dim);
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.task-item.bound {
  background: var(--accent-soft);
  border-color: color-mix(in srgb, var(--accent) 35%, var(--border));
}

.task-item .notes {
  font-size: 0.75rem;
  color: var(--text-dim);
  margin-top: 0.1rem;
  white-space: pre-line;
}

.task-item .body.edit {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  cursor: auto;
}

.field-label {
  font-size: 0.75rem;
  color: var(--text-dim);
}

.tag {
  color: var(--accent);
  font-weight: 600;
}

.filters {
  gap: 0.3rem;
}
</style>
