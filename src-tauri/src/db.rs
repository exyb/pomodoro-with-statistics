use rusqlite::{params, Connection};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

const SCHEMA_VERSION: i64 = 1;

pub struct Db {
    pub conn: Mutex<Connection>,
}

impl Db {
    pub fn open(dir: &Path) -> Result<Db, String> {
        fs::create_dir_all(dir).map_err(|e| format!("无法创建数据目录: {e}"))?;
        let conn = Connection::open(dir.join("pomodoro.db")).map_err(|e| format!("无法打开数据库: {e}"))?;
        conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| e.to_string())?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(|e| e.to_string())?;
        let db = Db { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    /// Locks are only held for the duration of one short statement, so a poisoned
    /// lock (a panic inside a statement) must not brick the whole app.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.lock();
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if version < SCHEMA_VERSION {
            conn.execute_batch(
                r#"
                BEGIN;
                CREATE TABLE kv (
                  key   TEXT PRIMARY KEY,
                  value TEXT NOT NULL
                );
                CREATE TABLE tasks (
                  id                 INTEGER PRIMARY KEY AUTOINCREMENT,
                  title              TEXT    NOT NULL,
                  notes              TEXT    NOT NULL DEFAULT '',
                  planned_pomodoros  INTEGER NOT NULL DEFAULT 0,
                  completed_pomodoros INTEGER NOT NULL DEFAULT 0,
                  status             TEXT    NOT NULL DEFAULT 'todo',
                  position           INTEGER NOT NULL DEFAULT 0,
                  created_at         INTEGER NOT NULL,
                  updated_at         INTEGER NOT NULL,
                  completed_at       INTEGER
                );
                CREATE TABLE sessions (
                  id             INTEGER PRIMARY KEY AUTOINCREMENT,
                  kind           TEXT    NOT NULL,
                  task_id        INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
                  description    TEXT    NOT NULL DEFAULT '',
                  planned_seconds INTEGER  NOT NULL,
                  focus_seconds   INTEGER  NOT NULL DEFAULT 0,
                  paused_seconds  INTEGER  NOT NULL DEFAULT 0,
                  interruptions   INTEGER  NOT NULL DEFAULT 0,
                  started_at      INTEGER  NOT NULL,
                  ended_at        INTEGER,
                  status          TEXT     NOT NULL,
                  cycle_index     INTEGER  NOT NULL DEFAULT 0
                );
                CREATE INDEX idx_sessions_started ON sessions(started_at);
                CREATE INDEX idx_sessions_task ON sessions(task_id);
                CREATE TABLE plugin_state (
                  id    TEXT PRIMARY KEY,
                  state TEXT NOT NULL
                );
                PRAGMA user_version = 1;
                COMMIT;
                "#,
            )
            .map_err(|e| format!("数据库迁移失败: {e}"))?;
        }
        Ok(())
    }

    pub fn kv_get(&self, key: &str) -> Option<String> {
        self.lock()
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| r.get(0))
            .ok()
    }

    pub fn kv_set(&self, key: &str, value: &str) {
        let _ = self.lock().execute(
            "INSERT INTO kv(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        );
    }

    pub fn plugin_state(&self, id: &str) -> Option<String> {
        self.lock()
            .query_row("SELECT state FROM plugin_state WHERE id = ?1", params![id], |r| r.get(0))
            .ok()
    }

    pub fn plugin_state_set(&self, id: &str, state: &str) {
        let _ = self.lock().execute(
            "INSERT INTO plugin_state(id, state) VALUES(?1, ?2)
             ON CONFLICT(id) DO UPDATE SET state = excluded.state",
            params![id, state],
        );
    }

    pub fn now_ms() -> i64 {
        chrono::Local::now().timestamp_millis()
    }
}
