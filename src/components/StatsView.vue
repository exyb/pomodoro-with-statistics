<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import BarChart from './charts/BarChart.vue'
import DonutChart from './charts/DonutChart.vue'
import HourHeat from './charts/HourHeat.vue'
import { api } from '@/lib/api'
import { humanDuration, minutesOf, shortDuration, timeOfDay } from '@/lib/format'
import { PHASE_LABEL, type SessionRow, type StatsReport } from '@/lib/types'
import { useAppStore } from '@/stores/app'

const store = useAppStore()

const granularity = ref<'day' | 'week'>('day')
const count = ref(14)
const report = ref<StatsReport | null>(null)
const sessions = ref<SessionRow[]>([])
const loading = ref(false)

const dayCounts = [7, 14, 30]
const weekCounts = [4, 8, 12]
const counts = computed(() => (granularity.value === 'day' ? dayCounts : weekCounts))

const rangeLabel = computed(() => {
  if (!report.value) return ''
  const unit = granularity.value === 'day' ? '天' : '周'
  return `最近 ${report.value.buckets.length} ${unit}`
})

const buckets = computed(() => report.value?.buckets ?? [])
const focusMinutes = computed(() => buckets.value.map((b) => minutesOf(b.focusSeconds)))
const bucketLabels = computed(() => buckets.value.map((b) => (granularity.value === 'day' ? b.subLabel : b.label)))
const currentBucketIndex = computed(() => buckets.value.length - 1)

const summary = computed(() => report.value?.summary ?? null)

const donutSegments = computed(() => {
  const s = summary.value
  if (!s) return []
  return [
    { label: '专注', value: minutesOf(s.focusSeconds), color: 'var(--accent)' },
    { label: '休息', value: minutesOf(s.breakSeconds), color: 'var(--break)' },
  ]
})

const rangeSessions = computed(() => {
  const r = report.value
  if (!r) return []
  return sessions.value.filter((s) => s.startedAt >= r.rangeStartMs && s.startedAt < r.rangeEndMs)
})

async function reload() {
  loading.value = true
  try {
    const [stats, recent] = await Promise.all([api.stats(granularity.value, count.value), api.listSessions(200)])
    report.value = stats
    sessions.value = recent
  } catch (e) {
    store.flash(String(e))
  } finally {
    loading.value = false
  }
}

async function removeSession(row: SessionRow) {
  await api.deleteSession(row.id)
  await reload()
}

function pickGranularity(next: 'day' | 'week') {
  granularity.value = next
  count.value = next === 'day' ? 14 : 8
  void reload()
}

watch(count, () => void reload())

// The store reloads today's numbers whenever a session ends, which is our cue to refresh.
watch(
  () => store.today,
  () => void reload(),
)

onMounted(() => void reload())
</script>

<template>
  <div class="stats">
    <div class="row between toolbar">
      <div class="row">
        <button class="chip" :class="{ active: granularity === 'day' }" @click="pickGranularity('day')">按天</button>
        <button class="chip" :class="{ active: granularity === 'week' }" @click="pickGranularity('week')">按周</button>
        <button
          v-for="c in counts"
          :key="c"
          class="chip"
          :class="{ active: count === c }"
          @click="count = c"
        >
          {{ c }}{{ granularity === 'day' ? '天' : '周' }}
        </button>
      </div>
      <div class="row">
        <span class="hint">{{ rangeLabel }}</span>
        <button class="btn ghost" :disabled="loading" @click="reload()">刷新</button>
      </div>
    </div>

    <div v-if="summary" class="kpi-grid">
      <div class="kpi">
        <b>{{ minutesOf(summary.focusSeconds) }}</b>
        <span>专注（分钟）</span>
        <div class="foot">{{ humanDuration(summary.focusSeconds) }}</div>
      </div>
      <div class="kpi">
        <b>{{ summary.pomodoros }}</b>
        <span>完成番茄</span>
        <div class="foot">中断后放弃 {{ summary.aborted }} 次</div>
      </div>
      <div class="kpi">
        <b>{{ summary.interruptions }}</b>
        <span>中断次数</span>
        <div class="foot">平均每次番茄 {{ (summary.interruptions / Math.max(1, summary.pomodoros + summary.aborted)).toFixed(1) }} 次</div>
      </div>
      <div class="kpi">
        <b>{{ minutesOf(summary.interruptedSeconds) }}</b>
        <span>中断（分钟）</span>
        <div class="foot">占专注 {{ summary.focusSeconds ? Math.round((summary.interruptedSeconds / summary.focusSeconds) * 100) : 0 }}%</div>
      </div>
      <div class="kpi">
        <b>{{ minutesOf(summary.breakSeconds) }}</b>
        <span>休息（分钟）</span>
      </div>
      <div class="kpi">
        <b>{{ summary.distinctTasks }}</b>
        <span>涉及任务</span>
      </div>
      <div class="kpi">
        <b>{{ Math.round(summary.completionRate * 100) }}%</b>
        <span>完成率</span>
        <div class="foot">开始 {{ summary.pomodoros + summary.aborted }} 个计时</div>
      </div>
      <div class="kpi">
        <b>{{ summary.avgSessionMinutes.toFixed(1) }}</b>
        <span>单次平均（分钟）</span>
        <div class="foot">最忙 {{ summary.bestBucketLabel }} · {{ shortDuration(summary.bestFocusSeconds) }}</div>
      </div>
    </div>

    <div class="chart-grid">
      <section class="card">
        <h3>专注时长分布</h3>
        <p class="hint">每{{ granularity === 'day' ? '天' : '周' }}累计专注分钟，最后一格是当前{{ granularity === 'day' ? '天' : '周' }}</p>
        <BarChart
          :values="focusMinutes"
          :labels="bucketLabels"
          :active-index="currentBucketIndex"
          unit=" 分钟"
        />
      </section>
      <section class="card">
        <h3>专注 / 休息</h3>
        <p class="hint">时间都花在哪</p>
        <DonutChart :segments="donutSegments" center-label="分钟专注" :center-value="String(minutesOf(summary?.focusSeconds ?? 0))" />
      </section>
    </div>

    <section class="card">
      <h3>一天中的开始时段</h3>
      <p class="hint">按工作计时开始的小时统计</p>
      <HourHeat :heat="report?.hourHeat ?? []" />
    </section>

    <section class="card">
      <h3>任务明细</h3>
      <p class="hint">按专注时长排序，含临时描述</p>
      <p v-if="!report?.tasks.length" class="empty">这段时间还没有记录。</p>
      <table v-else class="data">
        <thead>
          <tr>
            <th>任务</th>
            <th class="num">番茄</th>
            <th class="num">专注</th>
            <th class="num">中断</th>
            <th>占比</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="task in report.tasks" :key="`${task.taskId ?? 'd'}-${task.title}`">
            <td>{{ task.title }}</td>
            <td class="num">{{ task.pomodoros }}</td>
            <td class="num">{{ shortDuration(task.focusSeconds) }}</td>
            <td class="num">{{ task.interruptions }}</td>
            <td class="bar-track">
              <i
                class="bar-fill"
                :style="{ width: `${Math.round((task.focusSeconds / Math.max(1, summary?.focusSeconds ?? 1)) * 100)}%` }"
              />
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="card">
      <h3>计时记录</h3>
      <p class="hint">最近 {{ rangeSessions.length }} 条属于当前区间，可删除误记</p>
      <p v-if="!rangeSessions.length" class="empty">区间内没有记录。</p>
      <table v-else class="data">
        <thead>
          <tr>
            <th>时间</th>
            <th>类型</th>
            <th>内容</th>
            <th class="num">计划</th>
            <th class="num">实际</th>
            <th class="num">中断</th>
            <th>状态</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in rangeSessions" :key="row.id">
            <td>{{ timeOfDay(row.startedAt) }}</td>
            <td>{{ PHASE_LABEL[row.kind] }}</td>
            <td class="wrap">{{ row.description || '—' }}</td>
            <td class="num">{{ shortDuration(row.plannedSeconds) }}</td>
            <td class="num">{{ shortDuration(row.focusSeconds) }}</td>
            <td class="num">{{ row.interruptions }}</td>
            <td :class="{ dim: row.status === 'aborted' }">
              {{ row.status === 'completed' ? '完成' : row.status === 'aborted' ? '放弃' : '进行中' }}
            </td>
            <td class="num">
              <button class="icon-btn" title="删除这条记录" @click="removeSession(row)">×</button>
            </td>
          </tr>
        </tbody>
      </table>
    </section>
  </div>
</template>

<style scoped>
.stats {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.toolbar .hint {
  font-size: 0.76rem;
  color: var(--text-dim);
}

table.data .wrap {
  max-width: 18rem;
  white-space: normal;
  word-break: break-word;
}

table.data .dim {
  color: var(--text-dim);
}

.bar-track {
  min-width: 4rem;
}

.bar-track i {
  display: block;
  height: 100%;
  border-radius: 999px;
  transition: width 200ms ease;
}
</style>
