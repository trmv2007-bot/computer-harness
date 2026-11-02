use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path).context("open task database")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                objective TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('queued','running','paused','completed','failed','cancelled')),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_tasks_state_created ON tasks(state, created_at);
             CREATE TABLE IF NOT EXISTS events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(task_id) REFERENCES tasks(id)
             );
             CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY,
                action TEXT NOT NULL,
                reason TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('pending','approved','denied')),
                created_at INTEGER NOT NULL
             );
             INSERT OR IGNORE INTO schema_meta(key,value) VALUES('schema_version','1');",
        )
        .context("initialize database schema")?;
        Ok(Self { conn })
    }

    pub fn create_task(&self, title: &str, objective: &str) -> Result<Task> {
        let title = title.trim();
        let objective = objective.trim();
        anyhow::ensure!(!title.is_empty(), "task title cannot be empty");
        anyhow::ensure!(!objective.is_empty(), "task objective cannot be empty");
        anyhow::ensure!(title.len() <= 240, "task title must be 240 bytes or fewer");
        let now = now();
        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.to_owned(),
            objective: objective.to_owned(),
            state: "queued".to_owned(),
            created_at: now,
            updated_at: now,
        };
        self.conn.execute(
            "INSERT INTO tasks(id,title,objective,state,created_at,updated_at) VALUES(?1,?2,?3,'queued',?4,?4)",
            params![task.id, task.title, task.objective, now],
        )?;
        self.record_event(&task.id, "task.created", &serde_json::to_string(&task)?)?;
        Ok(task)
    }

    pub fn list_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,title,objective,state,created_at,updated_at FROM tasks ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Task {
                id: r.get(0)?,
                title: r.get(1)?,
                objective: r.get(2)?,
                state: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("read tasks")
    }

    pub fn get_task(&self, id: &str) -> Result<Option<Task>> {
        self.conn
            .query_row(
                "SELECT id,title,objective,state,created_at,updated_at FROM tasks WHERE id=?1",
                [id],
                |r| {
                    Ok(Task {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        objective: r.get(2)?,
                        state: r.get(3)?,
                        created_at: r.get(4)?,
                        updated_at: r.get(5)?,
                    })
                },
            )
            .optional()
            .context("read task")
    }

    pub fn cancel_task(&self, id: &str) -> Result<bool> {
        let now = now();
        let changed = self.conn.execute(
            "UPDATE tasks SET state='cancelled',updated_at=?2 WHERE id=?1 AND state IN ('queued','running','paused')",
            params![id, now],
        )?;
        if changed > 0 {
            self.record_event(id, "task.cancelled", "{}")?;
        }
        Ok(changed > 0)
    }

    pub fn record_event(&self, task_id: &str, kind: &str, payload: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO events(task_id,event_type,payload,created_at) VALUES(?1,?2,?3,?4)",
            params![task_id, kind, payload, now()],
        )?;
        Ok(())
    }

    pub fn event_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?)
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_lifecycle_persists_and_cancels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fairy.sqlite3");
        let id;
        {
            let store = Store::open(&path).unwrap();
            let task = store
                .create_task("Build tests", "Add a persistent task test")
                .unwrap();
            id = task.id.clone();
            assert_eq!(task.state, "queued");
            assert_eq!(store.list_tasks().unwrap().len(), 1);
            assert!(store.cancel_task(&id).unwrap());
            assert!(!store.cancel_task(&id).unwrap());
        }
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.get_task(&id).unwrap().unwrap().state, "cancelled");
        assert_eq!(reopened.event_count().unwrap(), 2);
    }

    #[test]
    fn rejects_empty_task_fields() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        assert!(store.create_task(" ", "objective").is_err());
        assert!(store.create_task("title", " ").is_err());
    }
}
