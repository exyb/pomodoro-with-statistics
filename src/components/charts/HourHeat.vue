<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ heat: number[] }>()

const max = computed(() => Math.max(1, ...props.heat))

function intensity(value: number): number {
  if (value <= 0) return 0
  return 0.18 + 0.82 * (value / max.value)
}
</script>

<template>
  <div class="heat">
    <div class="cells">
      <div v-for="(value, hour) in props.heat" :key="hour" class="cell" :title="`${hour}:00 开始 ${value} 个专注`">
        <i :style="{ opacity: intensity(value) }" />
        <span>{{ hour }}</span>
      </div>
    </div>
    <div class="legend">
      <span>最忙时段 {{ max }} 次</span>
    </div>
  </div>
</template>

<style scoped>
.cells {
  display: grid;
  grid-template-columns: repeat(12, minmax(0, 1fr));
  gap: 0.3rem;
}

.cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.15rem;
}

.cell i {
  width: 100%;
  aspect-ratio: 1;
  border-radius: 0.25rem;
  background: var(--accent);
  border: 1px solid var(--border);
}

.cell span {
  font-size: 0.62rem;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.legend {
  margin-top: 0.4rem;
  font-size: 0.72rem;
  color: var(--text-dim);
}
</style>
