//! Extension surface for the timer.
//!
//! A plugin is a Rust component registered in [`Registry::builtin`]. It receives the
//! timer's lifecycle events and may return [`Prompt`]s, which the engine forwards to the
//! webview as `pomodoro://prompt` events; the frontend renders them into the slot named
//! by the prompt. Everything a plugin needs to configure itself is a JSON object, so new
//! plugins need no frontend changes to appear in the settings page.

pub mod wordcard;

use crate::config::Settings;
use crate::db::Db;
use crate::timer::Phase;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptSlot {
    /// Inline card inside the timer panel.
    Timer,
    /// Line under the countdown in the floating window.
    Floating,
    /// Standalone toast window/banner.
    Banner,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    /// Monotonic id so the UI can dismiss or replace a prompt.
    pub id: u64,
    pub plugin_id: String,
    pub slot: PromptSlot,
    /// Renderer key, e.g. `word` — the frontend maps it to a card component.
    pub kind: String,
    pub title: String,
    pub body: String,
    /// Auto-hide delay; 0 keeps the prompt until it is replaced.
    pub ttl_ms: i64,
    pub data: Value,
}

/// Event payload handed to plugins. The built-in plugins only read a subset, so the
/// unread fields are part of the extension contract rather than dead code.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct TickInfo {
    pub phase: Phase,
    pub remaining_ms: i64,
    pub elapsed_ms: i64,
    pub planned_ms: i64,
    pub description: String,
    pub task_id: Option<i64>,
    pub now_ms: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum PluginEvent {
    PhaseStarted { phase: Phase, planned_ms: i64 },
    Tick(TickInfo),
    PhaseCompleted { phase: Phase, focus_ms: i64 },
    PhaseAbandoned { phase: Phase, focus_ms: i64 },
    Idle,
}

pub trait PomodoroPlugin: Send {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    /// Keys the settings page renders as editable fields with their defaults.
    fn default_config(&self) -> Value;
    fn on_event(&mut self, event: &PluginEvent, config: &Value) -> Vec<Prompt>;
    /// Optional persistence hook, stored in the `plugin_state` table.
    fn dump(&self) -> Value {
        Value::Null
    }
    fn restore(&mut self, _state: Value) {}
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMeta {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub config: Value,
    pub default_config: Value,
}

pub struct Registry {
    plugins: Vec<Box<dyn PomodoroPlugin>>,
    counter: u64,
}

impl Registry {
    /// Register built-in plugins here; that is the whole extension point.
    pub fn builtin() -> Registry {
        Registry {
            plugins: vec![Box::new(wordcard::WordCard::new())],
            counter: 0,
        }
    }

    pub fn meta(&self, settings: &Settings) -> Vec<PluginMeta> {
        self.plugins
            .iter()
            .map(|p| {
                let stored = settings.plugins.get(p.id());
                PluginMeta {
                    id: p.id().to_string(),
                    name: p.name().to_string(),
                    description: p.description().to_string(),
                    enabled: stored.map(|s| s.enabled).unwrap_or(true),
                    config: json!({
                        "default": p.default_config(),
                        "current": stored.map(|s| s.config.clone()).unwrap_or(Value::Null),
                    }),
                    default_config: p.default_config(),
                }
            })
            .collect()
    }

    /// Fan an event out to enabled plugins.
    pub fn dispatch(&mut self, event: &PluginEvent, settings: &Settings, db: &Db) -> Vec<Prompt> {
        let mut out = Vec::new();
        let mut id = self.counter;
        for plugin in &mut self.plugins {
            let enabled = settings
                .plugins
                .get(plugin.id())
                .map(|s| s.enabled)
                .unwrap_or(true);
            if !enabled {
                continue;
            }
            let cfg = Self::merged(plugin.as_ref(), settings);
            for mut prompt in plugin.on_event(event, &cfg) {
                id += 1;
                prompt.id = id;
                out.push(prompt);
            }
        }
        self.counter = id;
        if !out.is_empty() {
            self.save_states(db);
        }
        out
    }

    fn merged(plugin: &dyn PomodoroPlugin, settings: &Settings) -> Value {
        let base = plugin.default_config();
        match settings.plugins.get(plugin.id()) {
            Some(store) => merge_config(&base, &store.config),
            None => base,
        }
    }

    pub fn save_states(&mut self, db: &Db) {
        for plugin in &mut self.plugins {
            let state = plugin.dump();
            if state != Value::Null {
                db.plugin_state_set(plugin.id(), &state.to_string());
            }
        }
    }

    pub fn load_states(&mut self, db: &Db) {
        for plugin in &mut self.plugins {
            if let Some(raw) = db.plugin_state(plugin.id()) {
                if let Ok(value) = serde_json::from_str(&raw) {
                    plugin.restore(value);
                }
            }
        }
    }
}

/// Stored config wins over defaults, and unknown keys survive.
fn merge_config(defaults: &Value, stored: &Value) -> Value {
    let mut map = match defaults {
        Value::Object(m) => m.clone(),
        _ => serde_json::Map::new(),
    };
    if let Value::Object(stored) = stored {
        for (k, v) in stored {
            map.insert(k.clone(), v.clone());
        }
    }
    Value::Object(map)
}

