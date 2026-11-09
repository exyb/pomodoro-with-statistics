use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Whole-app configuration, persisted as a single JSON blob in the `kv` table.
/// `#[serde(default)]` on every container keeps older databases readable after new
/// fields are added, so no schema migration is needed for settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub timers: TimerSettings,
    pub behaviour: BehaviourSettings,
    pub appearance: AppearanceSettings,
    pub alerts: AlertSettings,
    pub floating: FloatingSettings,
    pub plugins: BTreeMap<String, PluginSettings>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TimerSettings {
    pub work_minutes: u32,
    pub short_break_minutes: u32,
    pub long_break_minutes: u32,
    /// Number of completed work sessions before a long break.
    pub long_break_interval: u32,
    /// Quick durations offered in the timer UI.
    pub work_presets: Vec<u32>,
    pub break_presets: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BehaviourSettings {
    pub auto_start_breaks: bool,
    pub auto_start_work: bool,
    /// Hide the window instead of quitting when the close button is pressed.
    pub close_to_tray: bool,
    /// Reset the pomodoro cycle when a work timer is started manually.
    pub reset_cycle_on_manual_start: bool,
    /// Register the app in the desktop session's autostart list.
    pub start_at_login: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppearanceSettings {
    /// light | dark | system
    pub theme: String,
    /// Multiplier applied to the root font size; the whole UI scales from it.
    pub ui_scale: f64,
    /// Empty string means the built-in accent colour.
    pub accent: String,
    pub show_seconds: bool,
    pub compact_layout: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AlertSettings {
    pub sound_enabled: bool,
    /// 0..=100
    pub volume: u32,
    /// Empty means the built-in synthesised chime.
    pub work_sound_path: String,
    pub break_sound_path: String,
    pub desktop_notification: bool,
    pub tray_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FloatingSettings {
    pub enabled: bool,
    /// 0.3..=1.0
    pub opacity: f64,
    pub click_through: bool,
    pub scale: f64,
    pub show_progress: bool,
    pub show_prompts: bool,
    pub position: Option<[i32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PluginSettings {
    pub enabled: bool,
    pub config: serde_json::Value,
}

impl Default for TimerSettings {
    fn default() -> Self {
        Self {
            work_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            long_break_interval: 4,
            work_presets: vec![15, 20, 25, 30, 45, 50],
            break_presets: vec![5, 10, 15, 20],
        }
    }
}

impl Default for BehaviourSettings {
    fn default() -> Self {
        Self {
            auto_start_breaks: true,
            auto_start_work: false,
            close_to_tray: true,
            reset_cycle_on_manual_start: false,
            start_at_login: false,
        }
    }
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            ui_scale: 1.0,
            accent: String::new(),
            show_seconds: true,
            compact_layout: false,
        }
    }
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            volume: 70,
            work_sound_path: String::new(),
            break_sound_path: String::new(),
            desktop_notification: true,
            tray_enabled: true,
        }
    }
}

impl Default for FloatingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            opacity: 0.92,
            click_through: false,
            scale: 1.0,
            show_progress: true,
            show_prompts: true,
            position: None,
        }
    }
}

impl Settings {
    pub fn clamp(&mut self) {
        self.timers.work_minutes = self.timers.work_minutes.clamp(1, 180);
        self.timers.short_break_minutes = self.timers.short_break_minutes.clamp(1, 60);
        self.timers.long_break_minutes = self.timers.long_break_minutes.clamp(1, 90);
        self.timers.long_break_interval = self.timers.long_break_interval.clamp(2, 12);
        self.appearance.ui_scale = self.appearance.ui_scale.clamp(0.7, 1.8);
        self.alerts.volume = self.alerts.volume.min(100);
        self.floating.opacity = self.floating.opacity.clamp(0.3, 1.0);
        self.floating.scale = self.floating.scale.clamp(0.7, 1.6);
    }

    pub fn duration_ms(&self, phase: crate::timer::Phase) -> i64 {
        let minutes = match phase {
            crate::timer::Phase::Work => self.timers.work_minutes,
            crate::timer::Phase::ShortBreak => self.timers.short_break_minutes,
            crate::timer::Phase::LongBreak => self.timers.long_break_minutes,
        };
        minutes as i64 * 60_000
    }
}
