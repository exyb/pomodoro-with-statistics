//! The `invoke` surface. Multi-field arguments travel as structs so the JS side always
//! sends camelCase keys and the Rust side owns the parsing.

use crate::app::Shared;
use crate::config::Settings;
use crate::engine;
use crate::plugins::PluginMeta;
use crate::session::SessionRow;
use crate::sound;
use crate::stats::{self, StatsReport};
use crate::tasks::{Task, TaskPatch};
use crate::timer::{Phase, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

fn core(state: State<'_, Shared>) -> Shared {
    state.inner().clone()
}

fn parse_phase(raw: Option<&str>) -> Phase {
    match raw {
        Some("shortBreak") => Phase::ShortBreak,
        Some("longBreak") => Phase::LongBreak,
        _ => Phase::Work,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub settings: Settings,
    pub snapshot: Snapshot,
    pub tasks: Vec<Task>,
    pub plugins: Vec<PluginMeta>,
    pub sound_backend: Option<String>,
    pub floating_visible: bool,
    pub platform: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartArgs {
    pub phase: Option<String>,
    pub minutes: Option<u32>,
    pub description: Option<String>,
    pub task_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextArgs {
    pub description: String,
    pub task_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DurationArgs {
    pub minutes: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSaveArgs {
    pub id: Option<i64>,
    pub title: String,
    pub notes: Option<String>,
    pub planned_pomodoros: Option<i32>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdArgs {
    pub id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdsArgs {
    pub ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitArgs {
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsArgs {
    pub granularity: String,
    pub count: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginArgs {
    pub id: String,
    pub enabled: Option<bool>,
    pub config: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundArgs {
    pub kind: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleArgs {
    pub on: bool,
}

#[tauri::command]
pub fn bootstrap(app: AppHandle, state: State<'_, Shared>) -> Bootstrap {
    let ac = core(state);
    let settings = ac.settings_snapshot();
    let registry = ac.plugins.lock().unwrap_or_else(|e| e.into_inner());
    Bootstrap {
        plugins: registry.meta(&settings),
        settings,
        snapshot: ac.timer.snapshot(),
        tasks: ac.db.task_list(),
        sound_backend: sound::available_backend(),
        floating_visible: app
            .get_webview_window(crate::windows::FLOATING)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false),
        platform: std::env::consts::OS.to_string(),
    }
}

#[tauri::command]
pub fn timer_start(app: AppHandle, state: State<'_, Shared>, args: StartArgs) -> Result<Snapshot, String> {
    let _ = app;
    let ac = core(state);
    let phase = parse_phase(args.phase.as_deref());
    engine::start(&ac, phase, args.minutes, args.description.unwrap_or_default(), args.task_id)
}

#[tauri::command]
pub fn timer_pause(state: State<'_, Shared>) -> Result<Snapshot, String> {
    engine::pause(&core(state))
}

#[tauri::command]
pub fn timer_resume(state: State<'_, Shared>) -> Result<Snapshot, String> {
    engine::resume(&core(state))
}

#[tauri::command]
pub fn timer_stop(state: State<'_, Shared>) -> Result<Snapshot, String> {
    engine::stop(&core(state))
}

#[tauri::command]
pub fn timer_skip(state: State<'_, Shared>) -> Result<Snapshot, String> {
    engine::skip(&core(state))
}

#[tauri::command]
pub fn timer_set_duration(state: State<'_, Shared>, args: DurationArgs) -> Snapshot {
    engine::set_duration(&core(state), args.minutes)
}

#[tauri::command]
pub fn timer_set_context(state: State<'_, Shared>, args: ContextArgs) -> Snapshot {
    engine::set_context(&core(state), args.description, args.task_id)
}

#[tauri::command]
pub fn timer_state(state: State<'_, Shared>) -> Snapshot {
    core(state).timer.snapshot()
}

#[tauri::command]
pub fn settings_save(app: AppHandle, state: State<'_, Shared>, settings: Settings) -> Result<Settings, String> {
    let ac = core(state);
    let previous = ac.settings_snapshot();
    let saved = ac.save_settings(settings)?;
    crate::apply_settings(&app, &previous, &saved);
    app.emit("pomodoro://settings-changed", saved.clone())
        .map_err(|e| e.to_string())?;
    Ok(saved)
}

#[tauri::command]
pub fn settings_test_sound(state: State<'_, Shared>, args: SoundArgs) {
    let ac = core(state);
    let kind = match args.kind.as_deref() {
        Some("shortBreak") => sound::SoundKind::ShortBreak,
        Some("longBreak") => sound::SoundKind::LongBreak,
        _ => sound::SoundKind::Work,
    };
    sound::play(&ac, kind);
}

#[tauri::command]
pub fn tasks_list(state: State<'_, Shared>) -> Vec<Task> {
    core(state).db.task_list()
}

#[tauri::command]
pub fn tasks_save(app: AppHandle, state: State<'_, Shared>, args: TaskSaveArgs) -> Result<Task, String> {
    let ac = core(state);
    let title = args.title.trim();
    if title.is_empty() {
        return Err("任务标题不能为空".into());
    }
    let notes = args.notes.unwrap_or_default();
    let planned = args.planned_pomodoros.unwrap_or(0).clamp(0, 40);
    let task = match args.id {
        Some(id) => {
            let patch = TaskPatch {
                title: Some(title.to_string()),
                notes: Some(notes),
                planned_pomodoros: Some(planned),
                status: args.status,
            };
            ac.db.task_update(id, &patch).ok_or("任务不存在")?
        }
        None => ac
            .db
            .task_create(title, &notes, planned)
            .ok_or("任务创建失败")?,
    };
    let _ = app.emit("pomodoro://tasks-changed", ());
    Ok(task)
}

#[tauri::command]
pub fn tasks_delete(app: AppHandle, state: State<'_, Shared>, args: IdArgs) {
    let ac = core(state);
    ac.db.task_delete(args.id);
    let _ = app.emit("pomodoro://tasks-changed", ());
}

#[tauri::command]
pub fn tasks_reorder(app: AppHandle, state: State<'_, Shared>, args: IdsArgs) {
    let ac = core(state);
    ac.db.task_reorder(&args.ids);
    let _ = app.emit("pomodoro://tasks-changed", ());
}

#[tauri::command]
pub fn sessions_list(state: State<'_, Shared>, args: LimitArgs) -> Vec<SessionRow> {
    core(state).db.session_recent(args.limit.unwrap_or(60).clamp(1, 500))
}

#[tauri::command]
pub fn session_delete(app: AppHandle, state: State<'_, Shared>, args: IdArgs) {
    let ac = core(state);
    ac.db.session_delete(args.id);
    let _ = app.emit("pomodoro://data-changed", ());
}

#[tauri::command]
pub fn stats_query(state: State<'_, Shared>, args: StatsArgs) -> StatsReport {
    let ac = core(state);
    let granularity = if args.granularity == "week" { "week" } else { "day" };
    // A week view compares 8 weeks; a day view compares the last 14 days.
    let count = args.count.unwrap_or(if granularity == "week" { 8 } else { 14 });
    stats::query(&ac.db, granularity, count)
}

#[tauri::command]
pub fn plugins_list(state: State<'_, Shared>) -> Vec<PluginMeta> {
    let ac = core(state);
    let settings = ac.settings_snapshot();
    let registry = ac.plugins.lock().unwrap_or_else(|e| e.into_inner());
    registry.meta(&settings)
}

#[tauri::command]
pub fn plugin_update(app: AppHandle, state: State<'_, Shared>, args: PluginArgs) -> Result<Vec<PluginMeta>, String> {
    let ac = core(state);
    let previous = ac.settings_snapshot();
    let settings = ac.update_settings(|s| {
        let entry = s.plugins.entry(args.id.clone()).or_default();
        if let Some(enabled) = args.enabled {
            entry.enabled = enabled;
        }
        if let Some(config) = args.config {
            entry.config = config;
        }
    })?;
    crate::apply_settings(&app, &previous, &settings);
    let _ = app.emit("pomodoro://settings-changed", settings.clone());
    let meta = ac.plugins.lock().unwrap_or_else(|e| e.into_inner()).meta(&settings);
    Ok(meta)
}

#[tauri::command]
pub fn floating_show(app: AppHandle) -> Result<(), String> {
    crate::windows::ensure_floating(&app)
}

#[tauri::command]
pub fn floating_hide(app: AppHandle) {
    crate::windows::close_floating(&app);
}

#[tauri::command]
pub fn floating_click_through(app: AppHandle, state: State<'_, Shared>, args: ToggleArgs) {
    let ac = core(state);
    let _ = ac.update_settings(|s| s.floating.click_through = args.on);
    if let Some(win) = app.get_webview_window(crate::windows::FLOATING) {
        crate::windows::set_click_through(&win, args.on);
    }
}

#[tauri::command]
pub fn app_show_main(app: AppHandle) {
    crate::windows::show_main(&app);
}

#[tauri::command]
pub fn app_quit(app: AppHandle) {
    crate::windows::quit(&app);
}
