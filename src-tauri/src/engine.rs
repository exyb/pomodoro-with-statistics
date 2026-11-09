//! Timer orchestration: phase transitions, persistence, alerts, plugin fan-out.
//!
//! The engine is the only place that writes session rows for a live timer, so the
//! database and the in-memory core can never disagree about what just happened.

use crate::app::Shared;
use crate::plugins::{PluginEvent, TickInfo};
use crate::sound::{self, SoundKind};
use crate::timer::{Completed, Phase, RunStatus, Snapshot};
use serde::Serialize;
use tauri_plugin_notification::NotificationExt;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFinished {
    pub phase: Phase,
    /// completed | aborted
    pub status: String,
    pub focus_seconds: i64,
    pub interrupted_seconds: i64,
    pub interruptions: i32,
    pub description: String,
    pub task_id: Option<i64>,
    pub session_id: Option<i64>,
    pub cycle_index: i32,
}

fn minutes_to_ms(ac: &Shared, phase: Phase, minutes: Option<u32>) -> i64 {
    match minutes {
        Some(m) if (1..=180).contains(&m) => m as i64 * 60_000,
        _ => ac.settings_snapshot().duration_ms(phase),
    }
}

/// Start a phase. Any session still running is closed as aborted first.
pub fn start(
    ac: &Shared,
    phase: Phase,
    minutes: Option<u32>,
    description: String,
    task_id: Option<i64>,
) -> Result<Snapshot, String> {
    if ac.timer.snapshot().status != RunStatus::Idle {
        abort_current(ac, false);
    }
    let planned = minutes_to_ms(ac, phase, minutes);
    let description = description.trim().chars().take(300).collect::<String>();
    let session_id = ac
        .db
        .session_insert(phase, task_id, &description, planned, ac.timer.cycle_index())
        .ok_or("无法创建计时记录")?;

    if phase.is_work() {
        let settings = ac.settings_snapshot();
        if settings.behaviour.reset_cycle_on_manual_start {
            ac.timer.set_cycle(0);
        }
        ac.remember_work_context(&description, task_id);
    }

    let snapshot = ac.timer.begin(phase, planned, description, task_id, session_id, crate::db::Db::now_ms());
    *ac.last_emit_sec.lock().unwrap_or_else(|e| e.into_inner()) = -1;

    emit_tick(ac, &snapshot);
    dispatch(
        ac,
        PluginEvent::PhaseStarted {
            phase,
            planned_ms: planned,
        },
    );
    crate::tray::update(ac);
    Ok(snapshot)
}

pub fn pause(ac: &Shared) -> Result<Snapshot, String> {
    let snapshot = ac.timer.pause().ok_or("计时未在运行")?;
    emit_tick(ac, &snapshot);
    crate::tray::update(ac);
    Ok(snapshot)
}

pub fn resume(ac: &Shared) -> Result<Snapshot, String> {
    let snapshot = ac.timer.resume().ok_or("计时未处于暂停")?;
    emit_tick(ac, &snapshot);
    crate::tray::update(ac);
    Ok(snapshot)
}

/// Re-aim the armed phase (used when the user picks another preset while idle).
pub fn set_duration(ac: &Shared, minutes: u32) -> Snapshot {
    let updated = ac.timer.set_planned(minutes.max(1) as i64 * 60_000);
    emit_tick(ac, &updated);
    crate::tray::update(ac);
    updated
}

pub fn set_context(ac: &Shared, description: String, task_id: Option<i64>) -> Snapshot {
    let description = description.trim().chars().take(300).collect::<String>();
    let snapshot = ac.timer.set_context(description.clone(), task_id);
    if let Some(id) = snapshot.session_id {
        ac.db.session_set_description(id, &description, task_id);
    }
    if snapshot.phase.is_work() {
        ac.remember_work_context(&description, task_id);
    }
    emit_tick(ac, &snapshot);
    snapshot
}

/// Stop the timer without advancing the cycle; the same phase can be started again.
pub fn stop(ac: &Shared) -> Result<Snapshot, String> {
    let Some(current) = abort_current(ac, true) else {
        let settings = ac.settings_snapshot();
        let snapshot = ac.timer.idle_on(Phase::Work, settings.duration_ms(Phase::Work));
        emit_tick(ac, &snapshot);
        crate::tray::update(ac);
        dispatch(ac, PluginEvent::Idle);
        return Ok(snapshot);
    };
    let settings = ac.settings_snapshot();
    let phase = current.phase;
    let snapshot = ac.timer.idle_on(phase, settings.duration_ms(phase));
    emit_tick(ac, &snapshot);
    crate::tray::update(ac);
    dispatch(ac, PluginEvent::Idle);
    Ok(snapshot)
}

/// Skip: close the phase and jump to the next one, started immediately.
pub fn skip(ac: &Shared) -> Result<Snapshot, String> {
    let snapshot = ac.timer.snapshot();
    if snapshot.status == RunStatus::Idle {
        return start(ac, snapshot.phase, None, String::new(), None);
    }
    let next = match snapshot.phase {
        // A skipped work timer does not count toward the long-break cycle.
        Phase::Work => ac.next_break_phase(ac.timer.cycle_index()),
        _ => Phase::Work,
    };
    abort_current(ac, false);
    let (description, task_id) = if next.is_work() { ac.work_context() } else { (String::new(), None) };
    start(ac, next, None, description, task_id)
}

/// Close the live session as aborted and write it out.
fn abort_current(ac: &Shared, emit_event: bool) -> Option<Completed> {
    let c = ac.timer.finish()?;
    let status = "aborted";
    if let Some(id) = c.session_id {
        ac.db
            .session_finish(id, c.focus_ms, c.paused_ms, c.interruptions, status);
    }
    dispatch(ac, PluginEvent::PhaseAbandoned { phase: c.phase, focus_ms: c.focus_ms });
    if emit_event {
        ac.emit(
            "pomodoro://session-finished",
            SessionFinished {
                phase: c.phase,
                status: status.into(),
                focus_seconds: c.focus_ms / 1000,
                interrupted_seconds: c.paused_ms / 1000,
                interruptions: c.interruptions,
                description: c.description.clone(),
                task_id: c.task_id,
                session_id: c.session_id,
                cycle_index: c.cycle_index,
            },
        );
    }
    Some(c)
}

/// Called by the tick thread when the countdown reaches zero.
fn finish_naturally(ac: &Shared) {
    let Some(c) = ac.timer.finish() else { return };
    if let Some(id) = c.session_id {
        ac.db
            .session_finish(id, c.focus_ms, c.paused_ms, c.interruptions, "completed");
    }

    let settings = ac.settings_snapshot();
    let next = if c.phase.is_work() {
        let cycle = ac.timer.add_cycle(1);
        if let Some(task_id) = c.task_id {
            ac.db.task_bump_completed(task_id, 1);
        }
        ac.remember_work_context(&c.description, c.task_id);
        ac.next_break_phase(cycle)
    } else {
        Phase::Work
    };
    let auto = if next.is_work() {
        settings.behaviour.auto_start_work
    } else {
        settings.behaviour.auto_start_breaks
    };

    alert(ac, c.phase, &c);
    ac.emit(
        "pomodoro://session-finished",
        SessionFinished {
            phase: c.phase,
            status: "completed".into(),
            focus_seconds: c.focus_ms / 1000,
            interrupted_seconds: c.paused_ms / 1000,
            interruptions: c.interruptions,
            description: c.description.clone(),
            task_id: c.task_id,
            session_id: c.session_id,
            cycle_index: c.cycle_index,
        },
    );
    dispatch(ac, PluginEvent::PhaseCompleted { phase: c.phase, focus_ms: c.focus_ms });

    let (description, task_id) = if next.is_work() { ac.work_context() } else { (String::new(), None) };
    if auto {
        let _ = start(ac, next, None, description, task_id);
    } else {
        let snapshot = ac.timer.idle_on(next, settings.duration_ms(next));
        emit_tick(ac, &snapshot);
        crate::tray::update(ac);
        dispatch(ac, PluginEvent::Idle);
    }
}

pub fn tick(ac: &Shared) {
    let snapshot = ac.timer.snapshot();
    if snapshot.status == RunStatus::Idle {
        return;
    }
    if snapshot.status == RunStatus::Running && snapshot.elapsed_ms >= snapshot.planned_ms {
        finish_naturally(ac);
        return;
    }
    emit_tick(ac, &snapshot);
    if snapshot.status == RunStatus::Running {
        dispatch(
            ac,
            PluginEvent::Tick(TickInfo {
                phase: snapshot.phase,
                remaining_ms: snapshot.remaining_ms,
                elapsed_ms: snapshot.elapsed_ms,
                planned_ms: snapshot.planned_ms,
                description: snapshot.description.clone(),
                task_id: snapshot.task_id,
                now_ms: crate::db::Db::now_ms(),
            }),
        );
    }
}

/// One UI event per second plus every status change.
fn emit_tick(ac: &Shared, snapshot: &Snapshot) {
    let sec = snapshot.remaining_ms / 1000;
    let mut last = ac.last_emit_sec.lock().unwrap_or_else(|e| e.into_inner());
    if *last == sec && snapshot.status != RunStatus::Paused {
        return;
    }
    *last = sec;
    ac.emit("pomodoro://tick", snapshot.clone());
}

fn dispatch(ac: &Shared, event: PluginEvent) {
    let settings = ac.settings_snapshot();
    let prompts = {
        let mut registry = ac.plugins.lock().unwrap_or_else(|e| e.into_inner());
        registry.dispatch(&event, &settings, &ac.db)
    };
    for prompt in prompts {
        ac.emit("pomodoro://prompt", prompt);
    }
}

fn alert(ac: &Shared, phase: Phase, c: &Completed) {
    let settings = ac.settings_snapshot();
    sound::play(ac, SoundKind::from_phase(phase));
    if !settings.alerts.desktop_notification {
        return;
    }
    let Some(handle) = ac.handle.get() else { return };
    let minutes = c.focus_ms / 60_000;
    let (title, body) = if phase.is_work() {
        let suffix = if c.description.is_empty() {
            format!("已完成 {} 个专注计时。", ac.timer.cycle_index())
        } else {
            format!("“{}” 的专注结束了。", c.description)
        };
        ("专注结束", format!("{} 休息一下吧。{suffix}", if minutes > 0 { format!("{minutes} 分钟") } else { "计时".into() }))
    } else {
        ("休息结束", "准备好开始下一个专注计时。".to_string())
    };
    let _ = handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

/// The loop that drives everything; runs on its own thread.
pub fn spawn_loop(ac: Shared) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        tick(&ac);
    });
}
