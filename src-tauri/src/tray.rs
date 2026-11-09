//! Status-notifier tray: always reflects the live timer and drives the main window.

use crate::app::Shared;
use crate::timer::{Phase, RunStatus};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

pub const TRAY_ID: &str = "main";

/// Menu labels are rebuilt only when they actually change (once a second at worst).
static LAST_LABELS: Mutex<Option<(String, String)>> = Mutex::new(None);

fn clock(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    format!("{:02}:{:02}", total / 60, total % 60)
}

pub fn describe(ac: &Shared) -> (String, String) {
    let snapshot = ac.timer.snapshot();
    let title = match snapshot.status {
        RunStatus::Idle => match snapshot.phase {
            Phase::Work => "番茄钟 · 待开始".to_string(),
            Phase::ShortBreak => "番茄钟 · 短休息待开始".to_string(),
            Phase::LongBreak => "番茄钟 · 长休息待开始".to_string(),
        },
        RunStatus::Running => format!("{} · 剩余 {}", snapshot.phase.label(), clock(snapshot.remaining_ms)),
        RunStatus::Paused => format!("{} · 已暂停 {}", snapshot.phase.label(), clock(snapshot.remaining_ms)),
    };
    let pause_label = match snapshot.status {
        RunStatus::Paused => "继续".to_string(),
        RunStatus::Running => "暂停".to_string(),
        RunStatus::Idle => "开始专注".to_string(),
    };
    (title, pause_label)
}

fn build_menu(app: &AppHandle, pause_label: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItemBuilder::with_id("show", "显示主窗口").build(app)?;
    let toggle = MenuItemBuilder::with_id("toggle", pause_label).build(app)?;
    let skip = MenuItemBuilder::with_id("skip", "跳过本阶段").build(app)?;
    let stop = MenuItemBuilder::with_id("stop", "停止计时").build(app)?;
    let floating = MenuItemBuilder::with_id("floating", "极简悬浮窗").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出番茄钟").build(app)?;
    let sep = PredefinedMenuItem::separator(app)?;
    MenuBuilder::new(app)
        .items(&[&show, &sep, &toggle, &skip, &stop, &sep, &floating, &sep, &quit])
        .build()
}

pub fn build(app: &AppHandle, ac: &Shared) -> tauri::Result<()> {
    let (_, pause_label) = describe(ac);
    let menu = build_menu(app, &pause_label)?;
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .icon(tauri::include_image!("icons/32x32.png"))
        .tooltip("番茄钟")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => crate::windows::show_main(app),
            "toggle" => crate::windows::toggle_from_tray(app),
            "skip" => crate::windows::command_from_tray(app, crate::windows::TrayCommand::Skip),
            "stop" => crate::windows::command_from_tray(app, crate::windows::TrayCommand::Stop),
            "floating" => crate::windows::toggle_floating(app),
            "quit" => crate::windows::quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                crate::windows::toggle_main(app);
            }
        })
        .build(app)?;
    let _ = tray;
    update(ac);
    Ok(())
}

/// Refresh tooltip and the 暂停/继续 label.
pub fn update(ac: &Shared) {
    let Some(app) = ac.handle.get() else { return };
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let (title, pause_label) = describe(ac);
    let previous = LAST_LABELS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if previous.as_ref() == Some(&(title.clone(), pause_label.clone())) {
        return;
    }
    *LAST_LABELS.lock().unwrap_or_else(|e| e.into_inner()) = Some((title.clone(), pause_label.clone()));
    let _ = tray.set_tooltip(Some(&title));
    if let Ok(menu) = build_menu(&app, &pause_label) {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn destroy(app: &AppHandle) {
    let _ = app.remove_tray_by_id(TRAY_ID);
}
