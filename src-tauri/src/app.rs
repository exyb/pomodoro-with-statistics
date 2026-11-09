use crate::config::Settings;
use crate::db::Db;
use crate::plugins::Registry;
use crate::timer::{Phase, TimerCore};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter};

/// Everything the commands and the tick thread share. Cloned into the thread as an `Arc`.
pub struct AppCore {
    pub db: Db,
    pub data_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub timer: TimerCore,
    pub plugins: Mutex<Registry>,
    /// Set once the Tauri app is built so core helpers can broadcast to every window.
    pub handle: OnceLock<AppHandle>,
    /// Last emitted whole-second remaining value, to keep tick events to 1/s.
    pub last_emit_sec: Mutex<i64>,
    /// Suggestion for the next work timer after an automatic cycle.
    pub last_work_context: Mutex<(String, Option<i64>)>,
}

pub type Shared = Arc<AppCore>;

impl AppCore {
    pub fn new(db: Db, data_dir: PathBuf, settings: Settings) -> Shared {
        Arc::new(AppCore {
            db,
            data_dir,
            settings: Mutex::new(settings),
            timer: TimerCore::default(),
            plugins: Mutex::new(Registry::builtin()),
            handle: OnceLock::new(),
            last_emit_sec: Mutex::new(-1),
            last_work_context: Mutex::new((String::new(), None)),
        })
    }

    fn state_lock(&self) -> std::sync::MutexGuard<'_, Settings> {
        self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn settings_snapshot(&self) -> Settings {
        self.state_lock().clone()
    }

    pub fn save_settings(&self, mut next: Settings) -> Result<Settings, String> {
        next.clamp();
        let json = serde_json::to_string(&next).map_err(|e| e.to_string())?;
        self.db.kv_set("settings", &json);
        *self.state_lock() = next.clone();
        Ok(next)
    }

    /// Mutate settings in place, persist, and hand back the saved copy; used by the
    /// engine, the tray and the plugin toggles.
    pub fn update_settings<F: FnOnce(&mut Settings)>(&self, f: F) -> Result<Settings, String> {
        let mut next = self.state_lock().clone();
        f(&mut next);
        self.save_settings(next)
    }

    pub fn load_settings(db: &Db) -> Settings {
        db.kv_get("settings")
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn sound_dir(&self) -> PathBuf {
        self.data_dir.join("sounds")
    }

    pub fn emit<T: Serialize + Clone>(&self, event: &str, payload: T) {
        if let Some(handle) = self.handle.get() {
            let _ = handle.emit(event, &payload);
        }
    }

    pub fn remember_work_context(&self, description: &str, task_id: Option<i64>) {
        let mut g = self.last_work_context.lock().unwrap_or_else(|e| e.into_inner());
        *g = (description.to_string(), task_id);
    }

    pub fn work_context(&self) -> (String, Option<i64>) {
        let g = self.last_work_context.lock().unwrap_or_else(|e| e.into_inner());
        g.clone()
    }

    /// Phase that should follow a finished work session.
    pub fn next_break_phase(&self, cycle_after_work: i32) -> Phase {
        let interval = self.settings_snapshot().timers.long_break_interval.max(1) as i32;
        if cycle_after_work > 0 && cycle_after_work % interval == 0 {
            Phase::LongBreak
        } else {
            Phase::ShortBreak
        }
    }
}
