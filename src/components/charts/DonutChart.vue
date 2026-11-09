<script setup lang="ts">
import { computed } from 'vue'

interface Segment {
  label: string
  value: number
  color: string
}

const props = defineProps<{
  segments: Segment[]
  centerLabel?: string
  centerValue?: string
}>()

const radius = 42
const circumference = 2 * Math.PI * radius

const total = computed(() => props.segments.reduce((sum, s) => sum + Math.max(0, s.value), 0))

const arcs = computed(() => {
  let offset = 0
  return props.segments.map((segment) => {
    const share = total.value > 0 ? Math.max(0, segment.value) / total.value : 0
    const arc = {
      segment,
      share,
      dash: `${(share * circumference).toFixed(2)} ${circumference.toFixed(2)}`,
      offset: (-offset * circumference).toFixed(2),
    }
    offset += share
    return arc
  })
})
</script>

<template>
  <div class="donut">
    <div class="ring">
      <svg viewBox="0 0 100 100">
        <circle cx="50" cy="50" :r="radius" fill="none" stroke="var(--ring-track)" stroke-width="12" />
        <g transform="rotate(-90 50 50)">
          <circle
            v-for="arc in arcs"
            :key="arc.segment.label"
            cx="50"
            cy="50"
            :r="radius"
            fill="none"
            :stroke="arc.segment.color"
            stroke-width="12"
            :stroke-dasharray="arc.dash"
            :stroke-dashoffset="arc.offset"
          >
            <title>{{ arc.segment.label }}：{{ Math.round(arc.share * 100) }}%</title>
          </circle>
        </g>
      </svg>
      <div class="center">
        <b>{{ centerValue }}</b>
        <span>{{ centerLabel }}</span>
      </div>
    </div>
    <ul class="legend">
      <li v-for="arc in arcs" :key="arc.segment.label">
        <i :style="{ background: arc.segment.color }" />
        <span class="name">{{ arc.segment.label }}</span>
        <span class="value">{{ arc.segment.value }}</span>
        <span class="share">{{ Math.round(arc.share * 100) }}%</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.donut {
  display: flex;
  align-items: center;
  gap: 0.9rem;
  flex-wrap: wrap;
}

.ring {
  position: relative;
  width: 7.5rem;
  aspect-ratio: 1;
  flex: none;
}

.ring svg {
  width: 100%;
  height: 100%;
}

.center {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  text-align: center;
}

.center b {
  font-size: 1.05rem;
  font-variant-numeric: tabular-nums;
}

.center span {
  font-size: 0.68rem;
  color: var(--text-dim);
}

.legend {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  font-size: 0.78rem;
  min-width: 8rem;
  flex: 1;
}

.legend li {
  display: grid;
  grid-template-columns: 0.6rem 1fr auto auto;
  align-items: center;
  gap: 0.4rem;
}

.legend i {
  width: 0.6rem;
  height: 0.6rem;
  border-radius: 0.2rem;
}

.legend .value {
  font-variant-numeric: tabular-nums;
}

.legend .share {
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}
</style>
