use crate::db::Db;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub notes: String,
    pub planned_pomodoros: i32,
    pub completed_pomodoros: i32,
    /// todo | doing | done
    pub status: String,
    pub position: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPatch {
    pub title: Option<String>,
    pub notes: Option<String>,
    pub planned_pomodoros: Option<i32>,
    pub status: Option<String>,
}

const SELECT_COLS: &str =
    "id, title, notes, planned_pomodoros, completed_pomodoros, status, position, created_at, updated_at, completed_at";

fn map_row(r: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        title: r.get(1)?,
        notes: r.get(2)?,
        planned_pomodoros: r.get(3)?,
        completed_pomodoros: r.get(4)?,
        status: r.get(5)?,
        position: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
        completed_at: r.get(9)?,
    })
}

impl Db {
    pub fn task_list(&self) -> Vec<Task> {
        let sql = format!("SELECT {SELECT_COLS} FROM tasks ORDER BY position ASC, id ASC");
        let conn = self.lock();
        let Ok(mut stmt) = conn.prepare(&sql) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map([], map_row) else {
            return Vec::new();
        };
        rows.filter_map(|x| x.ok()).collect()
    }

    pub fn task_get(&self, id: i64) -> Option<Task> {
        let sql = format!("SELECT {SELECT_COLS} FROM tasks WHERE id = ?1");
        self.lock()
            .query_row(&sql, params![id], map_row)
            .optional()
            .ok()
            .flatten()
    }

    pub fn task_create(&self, title: &str, notes: &str, planned: i32) -> Option<Task> {
        let now = Db::now_ms();
        let next_pos: i32 = self
            .lock()
            .query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM tasks", [], |r| r.get(0))
            .unwrap_or(0);
        let id = {
            let conn = self.lock();
            conn.execute(
                "INSERT INTO tasks(title, notes, planned_pomodoros, status, position, created_at, updated_at)
                 VALUES(?1, ?2, ?3, 'todo', ?4, ?5, ?5)",
                params![title, notes, planned, next_pos, now],
            )
            .ok()?;
            conn.last_insert_rowid()
        };
        self.task_get(id)
    }

    pub fn task_update(&self, id: i64, patch: &TaskPatch) -> Option<Task> {
        let current = self.task_get(id)?;
        let title = patch.title.clone().unwrap_or(current.title);
        let notes = patch.notes.clone().unwrap_or(current.notes);
        let planned = patch.planned_pomodoros.unwrap_or(current.planned_pomodoros);
        let status = patch.status.clone().unwrap_or(current.status);
        let completed_at = match status.as_str() {
            "done" => Some(current.completed_at.unwrap_or_else(Db::now_ms)),
            _ => None,
        };
        self.lock()
            .execute(
                "UPDATE tasks SET title=?1, notes=?2, planned_pomodoros=?3, status=?4, completed_at=?5, updated_at=?6 WHERE id=?7",
                params![title, notes, planned, status, completed_at, Db::now_ms(), id],
            )
            .ok()?;
        self.task_get(id)
    }

    pub fn task_delete(&self, id: i64) {
        let _ = self.lock().execute("DELETE FROM tasks WHERE id = ?1", params![id]);
    }

    pub fn task_reorder(&self, ordered_ids: &[i64]) {
        let conn = self.lock();
        for (idx, id) in ordered_ids.iter().enumerate() {
            let _ = conn.execute("UPDATE tasks SET position = ?1 WHERE id = ?2", params![idx as i32, id]);
        }
    }

    /// Count a finished pomodoro against the task (never below zero).
    pub fn task_bump_completed(&self, id: i64, delta: i32) {
        let _ = self.lock().execute(
            "UPDATE tasks SET completed_pomodoros = MAX(0, completed_pomodoros + ?1), updated_at = ?2 WHERE id = ?3",
            params![delta, Db::now_ms(), id],
        );
    }
}
