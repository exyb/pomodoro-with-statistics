import { markRaw, type Component } from 'vue'
import WordPrompt from './WordPrompt.vue'
import type { Prompt } from '@/lib/types'

/**
 * Frontend half of the plugin protocol. Rust prompts carry a `kind`; this map decides how
 * that kind is painted. A new plugin = one Rust plugin in `src-tauri/src/plugins/mod.rs`
 * plus one entry here (see docs/PLUGIN-API.md).
 */
export const promptRenderers: Record<string, Component> = {
  word: markRaw(WordPrompt),
}

export function rendererFor(prompt: Prompt): Component | null {
  return promptRenderers[prompt.kind] ?? null
}
