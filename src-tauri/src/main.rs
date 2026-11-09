#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod config;
mod db;
mod engine;
mod plugins;
mod session;
mod sound;
mod stats;
mod tasks;
mod timer;
mod tray;
mod windows;

use app::{AppCore, Shared};
use config::Settings;
use db::Db;
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;

/// Side effects that only matter when a setting changes, kept in one place so every
/// entry point (settings page, tray, plugin toggles) behaves the same.
fn apply_settings(app: &tauri::AppHandle, previous: &Settings, next: &Settings) {
    if next.alerts.tray_enabled && !previous.alerts.tray_enabled {
        if let Some(ac) = windows::shared(app) {
            let _ = tray::build(app, &ac);
        }
    } else if !next.alerts.tray_enabled && previous.alerts.tray_enabled {
        tray::destroy(app);
    }

    if next.floating.enabled && !previous.floating.enabled {
        let _ = windows::ensure_floating(app);
    } else if !next.floating.enabled && previous.floating.enabled {
        windows::close_floating(app);
    }

    if next.floating.click_through != previous.floating.click_through {
        if let Some(win) = app.get_webview_window(windows::FLOATING) {
            windows::set_click_through(&win, next.floating.click_through);
        }
    }

    if next.behaviour.start_at_login != previous.behaviour.start_at_login {
        let autolaunch = app.autolaunch();
        let _ = if next.behaviour.start_at_login {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
    }
}

fn main() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let db = Db::open(&data_dir).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            db.session_recover_orphans();
            let settings = AppCore::load_settings(&db);
            let ac = AppCore::new(db, data_dir, settings);
            let _ = ac.handle.set(handle.clone());
            app.manage(ac.clone());

            {
                let mut registry = ac.plugins.lock().unwrap_or_else(|e| e.into_inner());
                registry.load_states(&ac.db);
            }

            let current = ac.settings_snapshot();
            if current.alerts.tray_enabled {
                tray::build(&handle, &ac).map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?;
            }
            if current.floating.enabled {
                windows::ensure_floating(&handle).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            }
            if current.behaviour.start_at_login {
                let _ = handle.autolaunch().enable();
            }
            engine::spawn_loop(ac);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                let close_to_tray = window
                    .app_handle()
                    .try_state::<Shared>()
                    .map(|ac| ac.settings_snapshot().behaviour.close_to_tray)
                    .unwrap_or(false);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::timer_start,
            commands::timer_pause,
            commands::timer_resume,
            commands::timer_stop,
            commands::timer_skip,
            commands::timer_set_duration,
            commands::timer_set_context,
            commands::timer_state,
            commands::settings_save,
            commands::settings_test_sound,
            commands::tasks_list,
            commands::tasks_save,
            commands::tasks_delete,
            commands::tasks_reorder,
            commands::sessions_list,
            commands::session_delete,
            commands::stats_query,
            commands::plugins_list,
            commands::plugin_update,
            commands::floating_show,
            commands::floating_hide,
            commands::floating_click_through,
            commands::app_show_main,
            commands::app_quit,
        ]);

    let built = builder
        .build(tauri::generate_context!())
        .expect("番茄钟启动失败");

    built.run(move |app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            // The mini window can be left visible when the app is killed, so its last
            // position is banked here rather than only on close.
            if let Some(win) = app_handle.get_webview_window(windows::FLOATING) {
                windows::remember_floating_position(app_handle, &win);
            }
            if let Some(ac) = app_handle.try_state::<Shared>() {
                let mut registry = ac.plugins.lock().unwrap_or_else(|e| e.into_inner());
                registry.save_states(&ac.db);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::app::AppCore;
    use crate::config::Settings;
    use crate::db::Db;
    use crate::session;
    use crate::stats;
    use crate::tasks::TaskPatch;
    use crate::timer::{Phase, RunStatus, TimerCore};
    use std::path::PathBuf;

    fn temp_db(tag: &str) -> Db {
        let dir = std::env::temp_dir().join(format!("pomodoro-test-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Db::open(&dir).expect("open db")
    }

    #[test]
    fn settings_round_trip_and_clamp() {
        let mut s = Settings::default();
        s.timers.work_minutes = 400;
        s.appearance.ui_scale = 9.0;
        s.alerts.volume = 240;
        s.clamp();
        assert_eq!(s.timers.work_minutes, 180);
        assert_eq!(s.appearance.ui_scale, 1.8);
        assert_eq!(s.alerts.volume, 100);
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
        // Missing keys keep their defaults, so older files still load.
        let partial: Settings = serde_json::from_str(r#"{"timers":{"workMinutes":20}}"#).unwrap();
        assert_eq!(partial.timers.work_minutes, 20);
        assert_eq!(partial.timers.short_break_minutes, 5);
    }

    #[test]
    fn timer_accounts_pause_separately_from_focus() {
        let core = TimerCore::default();
        let snap = core.begin(Phase::Work, 60_000, "写周报".into(), Some(3), 1, Db::now_ms());
        assert_eq!(snap.status, RunStatus::Running);
        std::thread::sleep(std::time::Duration::from_millis(120));
        let paused = core.pause().expect("pause");
        assert_eq!(paused.interruptions, 1);
        let elapsed_before = paused.elapsed_ms;
        std::thread::sleep(std::time::Duration::from_millis(80));
        let resumed = core.resume().expect("resume");
        assert!(resumed.paused_ms >= 80, "paused time recorded: {}", resumed.paused_ms);
        assert_eq!(resumed.elapsed_ms, elapsed_before, "focus time must not grow while paused");
        std::thread::sleep(std::time::Duration::from_millis(50));
        let done = core.finish().expect("finish");
        assert!(done.focus_ms >= elapsed_before);
        assert!(core.snapshot().status == RunStatus::Idle);
        assert_eq!(core.snapshot().session_id, None);
    }

    #[test]
    fn cycle_and_long_break_cadence() {
        let mut settings = Settings::default();
        settings.timers.long_break_interval = 4;
        let db = temp_db("cycle");
        let dir = PathBuf::from("/tmp/pomodoro-cycle-app");
        let ac = AppCore::new(db, dir, settings);
        assert_eq!(ac.next_break_phase(1), Phase::ShortBreak);
        assert_eq!(ac.next_break_phase(4), Phase::LongBreak);
        assert_eq!(ac.next_break_phase(5), Phase::ShortBreak);
        assert_eq!(ac.next_break_phase(8), Phase::LongBreak);
    }

    #[test]
    fn tasks_crud_and_pomodoro_counter() {
        let db = temp_db("tasks");
        let task = db.task_create("写文档", "初稿", 4).expect("create");
        assert_eq!(task.title, "写文档");
        assert_eq!(task.status, "todo");
        db.task_bump_completed(task.id, 1);
        db.task_bump_completed(task.id, 1);
        let updated = db
            .task_update(
                task.id,
                &TaskPatch {
                    status: Some("done".into()),
                    ..Default::default()
                },
            )
            .expect("update");
        assert_eq!(updated.completed_pomodoros, 2);
        assert!(updated.completed_at.is_some());
        db.task_bump_completed(task.id, -5);
        assert_eq!(db.task_get(task.id).unwrap().completed_pomodoros, 0);

        let other = db.task_create("回邮件", "", 1).expect("create");
        let order = db.task_list();
        assert_eq!(order[0].id, task.id);
        db.task_reorder(&[other.id, task.id]);
        assert_eq!(db.task_list()[0].id, other.id);
    }

    #[test]
    fn stats_bucket_by_local_day_and_week() {
        let db = temp_db("stats");
        let today_ms = Db::now_ms();
        let yesterday = today_ms - 26 * 3600 * 1000;
        let id = db
            .session_insert(Phase::Work, None, "写文档", 25 * 60_000, 1)
            .expect("insert");
        // session_finish takes millisecond values, the DB stores whole seconds.
        db.session_finish(id, 25 * 60 * 1000, 3 * 60 * 1000, 2, "completed");
        let break_id = db
            .session_insert(Phase::ShortBreak, None, "", 5 * 60_000, 1)
            .expect("insert break");
        db.session_finish(break_id, 5 * 60 * 1000, 0, 0, "completed");

        let day = stats::query(&db, "day", 7);
        assert_eq!(day.buckets.len(), 7);
        assert_eq!(day.summary.pomodoros, 1);
        assert_eq!(day.summary.focus_seconds, 25 * 60);
        assert_eq!(day.summary.interruptions, 2);
        assert_eq!(day.summary.interrupted_seconds, 3 * 60);
        assert_eq!(day.summary.break_seconds, 5 * 60);
        assert_eq!(day.buckets.last().unwrap().pomodoros, 1);

        // A session older than the window is excluded.
        let old = db
            .session_insert(Phase::Work, None, "旧任务", 10 * 60_000, 1)
            .expect("insert old");
        db.session_finish(old, 10 * 60 * 1000, 0, 0, "completed");
        let stale = {
            let conn = db.lock();
            conn.execute("UPDATE sessions SET started_at = ?1 WHERE id = ?2", rusqlite::params![yesterday, old])
                .unwrap();
            0
        };
        assert_eq!(stale, 0);
        // Yesterday is inside a 2-day window but outside a 1-day (today-only) window.
        let day2 = stats::query(&db, "day", 2);
        assert_eq!(day2.summary.pomodoros, 2, "yesterday session must count in a 2-day window");
        let today_only = stats::query(&db, "day", 1);
        assert_eq!(today_only.summary.pomodoros, 1, "old session must fall out of a today-only window");
        assert!(today_only.range_start_ms > yesterday, "window must start after the old session");

        let week = stats::query(&db, "week", 4);
        assert_eq!(week.buckets.len(), 4);
        assert_eq!(week.summary.pomodoros, 2, "both work sessions are inside a 4-week window");
        assert!(week.buckets.last().unwrap().key.contains("-W"));
    }

    #[test]
    fn aborted_sessions_count_separately() {
        let db = temp_db("aborted");
        let id = db.session_insert(Phase::Work, None, "半途而废", 25 * 60_000, 1).unwrap();
        db.session_finish(id, 8 * 60 * 1000, 60 * 1000, 1, "aborted");
        let report = stats::query(&db, "day", 1);
        assert_eq!(report.summary.pomodoros, 0);
        assert_eq!(report.summary.aborted, 1);
        assert_eq!(report.summary.focus_seconds, 8 * 60);
        assert_eq!(report.tasks[0].title, "半途而废");
    }

    #[test]
    fn orphans_from_a_crash_are_closed() {
        let db = temp_db("orphan");
        db.session_insert(Phase::Work, None, "崩溃时正在跑", 25 * 60_000, 1).unwrap();
        db.session_recover_orphans();
        let recent = db.session_recent(10);
        assert_eq!(recent[0].status, "aborted");
        assert!(recent[0].ended_at.is_some());
    }

    #[test]
    fn kind_names_are_stable_because_stats_filter_on_them() {
        assert_eq!(session::kind_name(Phase::Work), "work");
        assert_eq!(session::kind_name(Phase::ShortBreak), "shortBreak");
        assert_eq!(session::kind_name(Phase::LongBreak), "longBreak");
    }
}
