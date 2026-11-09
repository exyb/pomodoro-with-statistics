<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    progress: number
    stroke?: number
    color?: string
  }>(),
  { stroke: 7, color: 'var(--accent)' },
)

const radius = 45
const circumference = 2 * Math.PI * radius
const offset = () => circumference * (1 - Math.min(1, Math.max(0, props.progress)))
</script>

<template>
  <svg viewBox="0 0 100 100" aria-hidden="true">
    <circle cx="50" cy="50" :r="radius" fill="none" stroke="var(--ring-track)" :stroke-width="stroke" />
    <circle
      cx="50"
      cy="50"
      :r="radius"
      fill="none"
      :stroke="color"
      :stroke-width="stroke"
      stroke-linecap="round"
      :stroke-dasharray="circumference"
      :stroke-dashoffset="offset()"
    />
  </svg>
</template>

<style scoped>
circle:last-of-type {
  transition: stroke-dashoffset 220ms linear, stroke 200ms ease;
}
</style>
