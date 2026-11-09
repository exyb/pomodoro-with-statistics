//! Built-in example plugin: vocabulary prompts during timers.
//!
//! It is the reference implementation for the extension interface — a plugin that keeps
//! its own state, reads a JSON config from the settings store, and emits prompts into the
//! timer and floating slots.

use super::{PluginEvent, PomodoroPlugin, Prompt, PromptSlot};
use crate::timer::Phase;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub en: String,
    pub zh: String,
    #[serde(default)]
    pub example: String,
}

const WORDS: &str = include_str!("./words.json");

pub struct WordCard {
    deck: Vec<Entry>,
    /// How often each word has been shown; the least-seen word in rotation wins.
    seen: HashMap<String, u32>,
    cursor: usize,
    last_emit_ms: i64,
}

impl WordCard {
    pub fn new() -> Self {
        let deck: Vec<Entry> = serde_json::from_str(WORDS).unwrap_or_default();
        WordCard {
            deck,
            seen: HashMap::new(),
            cursor: 0,
            last_emit_ms: 0,
        }
    }

    fn pick(&mut self) -> Option<Entry> {
        if self.deck.is_empty() {
            return None;
        }
        let total = self.deck.len();
        let mut best = self.cursor % total;
        for offset in 0..total {
            let idx = (self.cursor + offset) % total;
            if self.seen.get(&self.deck[idx].en).copied().unwrap_or(0)
                < self.seen.get(&self.deck[best].en).copied().unwrap_or(0)
            {
                best = idx;
            }
        }
        self.cursor = (best + 1) % total;
        *self.seen.entry(self.deck[best].en.clone()).or_insert(0) += 1;
        Some(self.deck[best].clone())
    }
}

impl PomodoroPlugin for WordCard {
    fn id(&self) -> &'static str {
        "wordcard"
    }

    fn name(&self) -> &'static str {
        "英语单词卡"
    }

    fn description(&self) -> &'static str {
        "在专注与休息过程中按间隔推送一个英语单词，词库可在 words.json 中替换。"
    }

    fn default_config(&self) -> Value {
        json!({
            "intervalSeconds": 90,
            "showDuringWork": true,
            "showDuringBreak": true,
            "showInFloating": true,
            "ttlSeconds": 12,
            "wordsPerPrompt": 1
        })
    }

    fn on_event(&mut self, event: &PluginEvent, config: &Value) -> Vec<Prompt> {
        let interval = config
            .get("intervalSeconds")
            .and_then(Value::as_i64)
            .unwrap_or(90)
            .max(10) * 1000;
        let ttl = config
            .get("ttlSeconds")
            .and_then(Value::as_i64)
            .unwrap_or(12)
            .max(3) * 1000;
        let in_work = config.get("showDuringWork").and_then(Value::as_bool).unwrap_or(true);
        let in_break = config.get("showDuringBreak").and_then(Value::as_bool).unwrap_or(true);
        let in_floating = config
            .get("showInFloating")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let per_prompt = config
            .get("wordsPerPrompt")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .clamp(1, 4) as usize;

        let (phase, now_ms, elapsed_ms) = match event {
            PluginEvent::Tick(t) => (t.phase, t.now_ms, t.elapsed_ms),
            PluginEvent::PhaseStarted { phase, .. } => (*phase, 0, 0),
            _ => return Vec::new(),
        };
        let allowed = match phase {
            Phase::Work => in_work,
            _ => in_break,
        };
        if !allowed || self.deck.is_empty() {
            return Vec::new();
        }
        // On a phase start the clock restarts, so the first word shows immediately;
        // during a tick we throttle by wall clock.
        let due = matches!(event, PluginEvent::PhaseStarted { .. }) || now_ms - self.last_emit_ms >= interval;
        if !due {
            return Vec::new();
        }
        self.last_emit_ms = now_ms;
        let _ = elapsed_ms;

        let words: Vec<Entry> = (0..per_prompt).filter_map(|_| self.pick()).collect();
        if words.is_empty() {
            return Vec::new();
        }
        let first = &words[0];
        let title = words
            .iter()
            .map(|w| w.en.clone())
            .collect::<Vec<_>>()
            .join(" · ");
        let body = words
            .iter()
            .map(|w| if w.example.is_empty() { w.zh.clone() } else { format!("{}｜{}", w.zh, w.example) })
            .collect::<Vec<_>>()
            .join("\n");

        let mut prompts = vec![Prompt {
            id: 0,
            plugin_id: self.id().to_string(),
            slot: PromptSlot::Timer,
            kind: "word".to_string(),
            title,
            body,
            ttl_ms: ttl,
            data: json!({ "words": words, "seen": self.seen.values().sum::<u32>() }),
        }];
        if in_floating {
            prompts.push(Prompt {
                id: 0,
                plugin_id: self.id().to_string(),
                slot: PromptSlot::Floating,
                kind: "word".to_string(),
                title: first.en.clone(),
                body: first.zh.clone(),
                ttl_ms: ttl,
                data: json!({ "words": [first] }),
            });
        }
        prompts
    }

    fn dump(&self) -> Value {
        json!({ "seen": self.seen, "cursor": self.cursor })
    }

    fn restore(&mut self, state: Value) {
        if let Some(map) = state.get("seen").and_then(Value::as_object) {
            for (k, v) in map {
                if let Some(n) = v.as_u64() {
                    self.seen.insert(k.clone(), n as u32);
                }
            }
        }
        if let Some(c) = state.get("cursor").and_then(Value::as_u64) {
            self.cursor = c as usize;
        }
    }
}
