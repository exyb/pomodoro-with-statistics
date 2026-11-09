<script setup lang="ts">
import { storeToRefs } from 'pinia'
import PromptRenderer from './PromptRenderer.vue'
import { useAppStore } from '@/stores/app'
import type { PromptSlot } from '@/lib/types'

const props = defineProps<{ slot: PromptSlot }>()
const store = useAppStore()
const { prompts } = storeToRefs(store)
</script>

<template>
  <div v-if="prompts.filter((p) => p.slot === props.slot).length" class="prompt-slot">
    <PromptRenderer
      v-for="prompt in prompts.filter((p) => p.slot === props.slot)"
      :key="prompt.id"
      :prompt="prompt"
      @dismiss="store.dismissPrompt"
    />
  </div>
</template>
