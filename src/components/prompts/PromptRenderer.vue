<script setup lang="ts">
import { computed } from 'vue'
import { rendererFor } from './registry'
import type { Prompt } from '@/lib/types'

const props = defineProps<{ prompt: Prompt }>()
const emit = defineEmits<{ dismiss: [id: number] }>()
const renderer = computed(() => rendererFor(props.prompt))
</script>

<template>
  <div v-if="renderer" class="prompt-card">
    <component :is="renderer" :prompt="props.prompt" />
    <button class="icon-btn close" title="关闭提示" @click="emit('dismiss', props.prompt.id)">×</button>
  </div>
  <div v-else class="prompt-card">
    <div>
      <div class="word">{{ props.prompt.title }}</div>
      <div class="body">{{ props.prompt.body }}</div>
    </div>
    <button class="icon-btn close" title="关闭提示" @click="emit('dismiss', props.prompt.id)">×</button>
  </div>
</template>

<style scoped>
.word {
  font-size: 1rem;
  font-weight: 700;
  color: var(--long-break);
}
.body {
  font-size: 0.85rem;
  white-space: pre-line;
}
</style>
