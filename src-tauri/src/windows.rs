//! Window plumbing: the floating mini window, tray-driven actions, close-to-tray.

use crate::app::Shared;
use crate::timer::RunStatus;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const FLOATING: &str = "floating";

pub fn shared(app: &AppHandle) -> Option<Shared> {
    app.try_state::<Shared>().map(|s| s.inner().clone())
}

fn floating_size(ac: &Shared) -> (f64, f64) {
    let settings = ac.settings_snapshot();
    let scale = settings.floating.scale.clamp(0.7, 1.6);
    let height = if settings.floating.show_prompts { 136.0 } else { 104.0 };
    (268.0 * scale, height * scale)
}

pub fn ensure_floating(app: &AppHandle) -> Result<(), String> {
    let ac = shared(app).ok_or("应用尚未初始化")?;
    let settings = ac.settings_snapshot();
    if let Some(win) = app.get_webview_window(FLOATING) {
        let _ = win.show();
        let _ = win.set_always_on_top(true);
        if let Some([x, y]) = settings.floating.position {
            let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
        }
        set_click_through(&win, settings.floating.click_through);
        return Ok(());
    }

    let (width, height) = floating_size(&ac);
    let mut builder = WebviewWindowBuilder::new(app, FLOATING, WebviewUrl::App("floating.html".into()))
        .title("番茄钟 · 极简")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .closable(true)
        .visible(true)
        .inner_size(width, height);
    builder = match settings.floating.position {
        Some([x, y]) => builder.position(x as f64, y as f64),
        None => builder.center(),
    };
    let win = builder.build().map_err(|e| e.to_string())?;
    let _ = win.set_always_on_top(true);
    set_click_through(&win, settings.floating.click_through);
    Ok(())
}

pub fn remember_floating_position(app: &AppHandle, win: &WebviewWindow) {
    let Some(ac) = shared(app) else { return };
    let Ok(pos) = win.outer_position() else { return };
    let _ = ac.update_settings(|s| s.floating.position = Some([pos.x, pos.y]));
}

pub fn set_click_through(win: &WebviewWindow, on: bool) {
    let _ = win.set_ignore_cursor_events(on);
}

/// Called when the mini window's close button is pressed: turn the feature off so the
/// next start does not resurrect it.
pub fn close_floating(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(FLOATING) {
        remember_floating_position(app, &win);
    }
    if let Some(ac) = shared(app) {
        if let Ok(settings) = ac.update_settings(|s| s.floating.enabled = false) {
            let _ = app.emit("pomodoro://settings-changed", settings);
        }
    }
    if let Some(win) = app.get_webview_window(FLOATING) {
        // Destroy instead of close: the close request handler would re-enter itself.
        let _ = win.destroy();
    }
}

pub fn toggle_floating(app: &AppHandle) {
    let visible = app
        .get_webview_window(FLOATING)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if visible {
        close_floating(app);
        return;
    }
    if let Some(ac) = shared(app) {
        let _ = ac.update_settings(|s| s.floating.enabled = true);
    }
    let _ = ensure_floating(app);
}

pub fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

pub fn toggle_main(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    let visible = win.is_visible().unwrap_or(true);
    if visible && !win.is_focused().unwrap_or(false) {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    if visible {
        let _ = win.hide();
    } else {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

pub fn quit(app: &AppHandle) {
    app.exit(0);
}

pub enum TrayCommand {
    Skip,
    Stop,
}

/// The tray drives the same state machine as the UI, so it calls the engine directly.
pub fn command_from_tray(app: &AppHandle, cmd: TrayCommand) {
    let Some(ac) = shared(app) else { return };
    let _ = match cmd {
        TrayCommand::Skip => crate::engine::skip(&ac),
        TrayCommand::Stop => crate::engine::stop(&ac),
    };
}

pub fn toggle_from_tray(app: &AppHandle) {
    let Some(ac) = shared(app) else { return };
    let snapshot = ac.timer.snapshot();
    match snapshot.status {
        RunStatus::Running => {
            let _ = crate::engine::pause(&ac);
        }
        RunStatus::Paused => {
            let _ = crate::engine::resume(&ac);
        }
        RunStatus::Idle => {
            let (description, task_id) = if snapshot.phase.is_work() {
                ac.work_context()
            } else {
                (String::new(), None)
            };
            let _ = crate::engine::start(&ac, snapshot.phase, None, description, task_id);
        }
    }
}
