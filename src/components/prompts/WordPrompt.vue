<script setup lang="ts">
import { computed } from 'vue'
import type { Prompt } from '@/lib/types'

const props = defineProps<{ prompt: Prompt }>()

const words = computed(() => props.prompt.data?.words ?? [])
</script>

<template>
  <div class="word-prompt">
    <div v-if="words.length === 1" class="one">
      <div class="word">{{ words[0].en }}</div>
      <div class="body">{{ words[0].zh }}</div>
      <div v-if="words[0].example" class="meta">{{ words[0].example }}</div>
    </div>
    <div v-else class="many">
      <div class="word">{{ prompt.title }}</div>
      <div class="body">{{ prompt.body }}</div>
    </div>
    <div v-if="prompt.data?.seen" class="meta">已学习 {{ prompt.data.seen }} 次</div>
  </div>
</template>

<style scoped>
.word-prompt {
  flex: 1;
  min-width: 0;
}
.word {
  font-size: 1.15rem;
  font-weight: 700;
  color: var(--long-break);
  letter-spacing: 0.01em;
}
.body {
  font-size: 0.88rem;
  white-space: pre-line;
}
.meta {
  font-size: 0.74rem;
  color: var(--text-dim);
  font-style: italic;
}
</style>
