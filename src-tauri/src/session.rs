use crate::db::Db;
use crate::timer::Phase;
use rusqlite::{params, Row};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: i64,
    pub kind: String,
    pub task_id: Option<i64>,
    pub description: String,
    pub planned_seconds: i64,
    pub focus_seconds: i64,
    pub paused_seconds: i64,
    pub interruptions: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// running | completed | aborted
    pub status: String,
    pub cycle_index: i64,
}

const SELECT_COLS: &str = "id, kind, task_id, description, planned_seconds, focus_seconds, paused_seconds, interruptions, started_at, ended_at, status, cycle_index";

fn map_row(r: &Row<'_>) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        id: r.get(0)?,
        kind: r.get(1)?,
        task_id: r.get(2)?,
        description: r.get(3)?,
        planned_seconds: r.get(4)?,
        focus_seconds: r.get(5)?,
        paused_seconds: r.get(6)?,
        interruptions: r.get(7)?,
        started_at: r.get(8)?,
        ended_at: r.get(9)?,
        status: r.get(10)?,
        cycle_index: r.get(11)?,
    })
}

pub fn kind_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Work => "work",
        Phase::ShortBreak => "shortBreak",
        Phase::LongBreak => "longBreak",
    }
}

impl Db {
    /// A session row is created as soon as the timer starts so a crash still leaves a record.
    pub fn session_insert(&self, phase: Phase, task_id: Option<i64>, description: &str, planned_ms: i64, cycle_index: i32) -> Option<i64> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO sessions(kind, task_id, description, planned_seconds, started_at, status, cycle_index)
             VALUES(?1, ?2, ?3, ?4, ?5, 'running', ?6)",
            params![kind_name(phase), task_id, description, planned_ms / 1000, Db::now_ms(), cycle_index],
        )
        .ok()?;
        Some(conn.last_insert_rowid())
    }

    pub fn session_set_description(&self, id: i64, description: &str, task_id: Option<i64>) {
        let _ = self.lock().execute(
            "UPDATE sessions SET description = ?2, task_id = ?3 WHERE id = ?1",
            params![id, description, task_id],
        );
    }

    pub fn session_finish(&self, id: i64, focus_ms: i64, paused_ms: i64, interruptions: i32, status: &str) {
        let _ = self.lock().execute(
            "UPDATE sessions SET focus_seconds=?2, paused_seconds=?3, interruptions=?4, ended_at=?5, status=?6 WHERE id=?1",
            params![id, focus_ms / 1000, paused_ms / 1000, interruptions, Db::now_ms(), status],
        );
    }

    /// Orphaned `running` rows from a previous process run are closed as aborted.
    pub fn session_recover_orphans(&self) {
        let _ = self.lock().execute(
            "UPDATE sessions SET status='aborted', ended_at=?1 WHERE status='running'",
            params![Db::now_ms()],
        );
    }

    pub fn session_recent(&self, limit: i64) -> Vec<SessionRow> {
        let conn = self.lock();
        let Ok(mut stmt) = conn.prepare(&format!("SELECT {SELECT_COLS} FROM sessions ORDER BY started_at DESC LIMIT ?1")) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![limit], map_row) else {
            return Vec::new();
        };
        rows.filter_map(|x| x.ok()).collect()
    }

    pub fn session_delete(&self, id: i64) {
        let _ = self.lock().execute("DELETE FROM sessions WHERE id = ?1", params![id]);
    }

    /// All sessions whose start falls inside `[from_ms, to_ms)`.
    pub fn sessions_in(&self, from_ms: i64, to_ms: i64) -> Vec<SessionRow> {
        let conn = self.lock();
        let Ok(mut stmt) = conn.prepare(&format!(
            "SELECT {SELECT_COLS} FROM sessions WHERE started_at >= ?1 AND started_at < ?2 ORDER BY started_at ASC"
        )) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![from_ms, to_ms], map_row) else {
            return Vec::new();
        };
        rows.filter_map(|x| x.ok()).collect()
    }
}
