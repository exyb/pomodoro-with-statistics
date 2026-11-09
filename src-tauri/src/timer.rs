use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Work,
    ShortBreak,
    LongBreak,
}

impl Default for Phase {
    fn default() -> Self {
        Phase::Work
    }
}

impl Phase {
    pub fn is_work(self) -> bool {
        matches!(self, Phase::Work)
    }

    pub fn label(self) -> &'static str {
        match self {
            Phase::Work => "专注",
            Phase::ShortBreak => "短休息",
            Phase::LongBreak => "长休息",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Idle,
    Running,
    Paused,
}

impl Default for RunStatus {
    fn default() -> Self {
        RunStatus::Idle
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: Phase,
    pub status: RunStatus,
    pub planned_ms: i64,
    pub elapsed_ms: i64,
    pub remaining_ms: i64,
    pub paused_ms: i64,
    pub interruptions: i32,
    pub cycle_index: i32,
    pub description: String,
    pub task_id: Option<i64>,
    pub session_id: Option<i64>,
    pub started_at_ms: Option<i64>,
    pub progress: f64,
}

#[derive(Clone, Debug)]
pub struct Completed {
    pub phase: Phase,
    pub focus_ms: i64,
    pub paused_ms: i64,
    pub interruptions: i32,
    pub session_id: Option<i64>,
    pub task_id: Option<i64>,
    pub description: String,
    pub cycle_index: i32,
}

#[derive(Default)]
struct Inner {
    phase: Phase,
    status: RunStatus,
    planned_ms: i64,
    /// Active time banked before the current running segment.
    active_ms: i64,
    segment_start: Option<Instant>,
    paused_ms: i64,
    pause_start: Option<Instant>,
    interruptions: i32,
    cycle_index: i32,
    description: String,
    task_id: Option<i64>,
    session_id: Option<i64>,
    started_at_ms: Option<i64>,
}

impl Inner {
    fn elapsed(&self) -> i64 {
        let running = self
            .segment_start
            .map(|s| s.elapsed().as_millis() as i64)
            .unwrap_or(0);
        self.active_ms + running
    }

    fn snapshot(&self) -> Snapshot {
        let elapsed = self.elapsed().min(self.planned_ms.max(0));
        let planned = self.planned_ms;
        Snapshot {
            phase: self.phase,
            status: self.status,
            planned_ms: planned,
            elapsed_ms: elapsed,
            remaining_ms: (planned - elapsed).max(0),
            paused_ms: self.paused_ms
                + self
                    .pause_start
                    .map(|s| s.elapsed().as_millis() as i64)
                    .unwrap_or(0),
            interruptions: self.interruptions,
            cycle_index: self.cycle_index,
            description: self.description.clone(),
            task_id: self.task_id,
            session_id: self.session_id,
            started_at_ms: self.started_at_ms,
            progress: if planned > 0 {
                (elapsed as f64 / planned as f64).clamp(0.0, 1.0)
            } else {
                0.0
            },
        }
    }
}

/// The authoritative timer. Wall-clock deltas are measured with `Instant`, so the
/// countdown never drifts with scheduling hiccups and both windows read one source.
#[derive(Default)]
pub struct TimerCore {
    inner: Mutex<Inner>,
}

impl TimerCore {
    pub fn snapshot(&self) -> Snapshot {
        self.lock().snapshot()
    }

    pub fn cycle_index(&self) -> i32 {
        self.lock().cycle_index
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Start a fresh phase; previous accumulators are dropped.
    pub fn begin(&self, phase: Phase, planned_ms: i64, description: String, task_id: Option<i64>, session_id: i64, started_at_ms: i64) -> Snapshot {
        let mut g = self.lock();
        g.phase = phase;
        g.status = RunStatus::Running;
        g.planned_ms = planned_ms;
        g.active_ms = 0;
        g.segment_start = Some(Instant::now());
        g.paused_ms = 0;
        g.pause_start = None;
        g.interruptions = 0;
        g.description = description;
        g.task_id = task_id;
        g.session_id = Some(session_id);
        g.started_at_ms = Some(started_at_ms);
        g.snapshot()
    }

    /// Park on a phase without running: the UI shows what would come next.
    pub fn idle_on(&self, phase: Phase, planned_ms: i64) -> Snapshot {
        let mut g = self.lock();
        g.phase = phase;
        g.status = RunStatus::Idle;
        g.planned_ms = planned_ms;
        g.active_ms = 0;
        g.segment_start = None;
        g.paused_ms = 0;
        g.pause_start = None;
        g.interruptions = 0;
        g.session_id = None;
        g.started_at_ms = None;
        g.snapshot()
    }

    pub fn pause(&self) -> Option<Snapshot> {
        let mut g = self.lock();
        if g.status != RunStatus::Running {
            return None;
        }
        g.active_ms = g.elapsed();
        g.segment_start = None;
        g.pause_start = Some(Instant::now());
        g.status = RunStatus::Paused;
        // One pause inside a work timer counts as one interruption.
        g.interruptions += 1;
        Some(g.snapshot())
    }

    pub fn resume(&self) -> Option<Snapshot> {
        let mut g = self.lock();
        if g.status != RunStatus::Paused {
            return None;
        }
        if let Some(start) = g.pause_start.take() {
            g.paused_ms += start.elapsed().as_millis() as i64;
        }
        g.segment_start = Some(Instant::now());
        g.status = RunStatus::Running;
        Some(g.snapshot())
    }

    /// Re-arm a paused timer with a new planned length (used by the duration presets
    /// while a timer is already running).
    pub fn set_planned(&self, planned_ms: i64) -> Snapshot {
        let mut g = self.lock();
        if g.status == RunStatus::Running {
            // Bank the running segment, then re-anchor so the delta is not counted twice.
            g.active_ms = g.elapsed();
            g.segment_start = Some(Instant::now());
        }
        g.planned_ms = planned_ms;
        g.snapshot()
    }

    pub fn set_context(&self, description: String, task_id: Option<i64>) -> Snapshot {
        let mut g = self.lock();
        g.description = description;
        g.task_id = task_id;
        g.snapshot()
    }

    pub fn add_cycle(&self, delta: i32) -> i32 {
        let mut g = self.lock();
        g.cycle_index = (g.cycle_index + delta).max(0);
        g.cycle_index
    }

    pub fn set_cycle(&self, value: i32) {
        self.lock().cycle_index = value.max(0);
    }

    /// Ends the current phase; the caller decides whether that counts as completed
    /// or aborted when it writes the session row.
    pub fn finish(&self) -> Option<Completed> {
        let mut g = self.lock();
        if g.status == RunStatus::Idle || g.session_id.is_none() {
            return None;
        }
        let c = Completed {
            phase: g.phase,
            focus_ms: g.elapsed(),
            paused_ms: g.paused_ms
                + g.pause_start.map(|s| s.elapsed().as_millis() as i64).unwrap_or(0),
            interruptions: g.interruptions,
            session_id: g.session_id,
            task_id: g.task_id,
            description: std::mem::take(&mut g.description),
            cycle_index: g.cycle_index,
        };
        g.status = RunStatus::Idle;
        g.active_ms = 0;
        g.segment_start = None;
        g.pause_start = None;
        g.paused_ms = 0;
        g.interruptions = 0;
        g.session_id = None;
        g.started_at_ms = None;
        Some(c)
    }
}
