<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    values: number[]
    labels: string[]
    color?: string
    /** Appended to the hover text, e.g. "分钟". */
    unit?: string
    /** Index highlighted (usually the current bucket). */
    activeIndex?: number | null
  }>(),
  { color: 'var(--accent)', unit: '', activeIndex: null },
)

const max = computed(() => Math.max(1, ...props.values))
/** Bars are laid out in a 0..100 viewBox that stretches, so any width scales. */
const step = computed(() => 100 / Math.max(props.values.length, 1))
const gap = computed(() => step.value * 0.28)

function height(value: number): number {
  if (value <= 0) return 0.6
  return Math.max(1.5, (value / max.value) * 100)
}
</script>

<template>
  <div class="bar-chart">
    <div class="plot">
      <svg viewBox="0 0 100 100" preserveAspectRatio="none">
        <line x1="0" y1="100" x2="100" y2="100" stroke="var(--border)" stroke-width="0.6" vector-effect="non-scaling-stroke" />
        <rect
          v-for="(value, i) in props.values"
          :key="i"
          :x="i * step + gap / 2"
          :y="100 - height(value)"
          :width="step - gap"
          :height="height(value)"
          :fill="color"
          :opacity="activeIndex === i ? 1 : 0.72"
          rx="0.8"
        >
          <title>{{ labels[i] }}：{{ value }}{{ unit }}</title>
        </rect>
      </svg>
    </div>
    <div class="axis">
      <span v-for="(label, i) in props.labels" :key="i" :class="{ active: activeIndex === i }">{{ label }}</span>
    </div>
  </div>
</template>

<style scoped>
.bar-chart {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.plot {
  height: 9.5rem;
}

.plot svg {
  width: 100%;
  height: 100%;
  overflow: visible;
}

rect {
  transition: height 220ms ease, y 220ms ease, opacity 150ms ease;
}

.axis {
  display: flex;
  font-size: 0.68rem;
  color: var(--text-dim);
}

.axis span {
  flex: 1;
  text-align: center;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.axis span.active {
  color: var(--accent);
  font-weight: 600;
}
</style>
