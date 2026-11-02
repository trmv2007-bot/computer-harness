use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityGrant {
    pub capability: String,
    pub note: String,
    pub granted_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Approval {
    pub id: String,
    pub action: String,
    pub reason: String,
    pub state: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskRun {
    pub id: String,
    pub task_id: String,
    pub prompt: String,
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub usage: Option<Value>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
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
            "BEGIN IMMEDIATE;
             CREATE TABLE IF NOT EXISTS schema_meta (
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
             CREATE TABLE IF NOT EXISTS capability_grants (
                capability TEXT PRIMARY KEY,
                note TEXT NOT NULL,
                granted_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS approval_uses (
                approval_id TEXT PRIMARY KEY,
                consumed_at INTEGER NOT NULL,
                FOREIGN KEY(approval_id) REFERENCES approvals(id)
             );
             CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS task_runs (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                prompt TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('running','succeeded','failed')),
                result TEXT,
                error TEXT,
                usage_json TEXT,
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                FOREIGN KEY(task_id) REFERENCES tasks(id)
             );
             CREATE INDEX IF NOT EXISTS idx_task_runs_task_started
                ON task_runs(task_id, started_at DESC);
             INSERT OR IGNORE INTO schema_meta(key,value) VALUES('schema_version','4');
             UPDATE schema_meta SET value='4'
                WHERE key='schema_version' AND CAST(value AS INTEGER)<4;
             COMMIT;",
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
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO tasks(id,title,objective,state,created_at,updated_at) VALUES(?1,?2,?3,'queued',?4,?4)",
            params![task.id, task.title, task.objective, now],
        )?;
        record_task_event_on(
            &tx,
            &task.id,
            "task.created",
            &serde_json::to_string(&task)?,
        )?;
        tx.commit()?;
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

    pub fn transition_task(&self, id: &str, next: &str) -> Result<Task> {
        let current = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found: {id}"))?;
        let allowed = match current.state.as_str() {
            "queued" => matches!(next, "paused" | "failed" | "cancelled"),
            "running" => false,
            "paused" => matches!(next, "queued" | "completed" | "failed" | "cancelled"),
            "failed" => next == "queued",
            "completed" | "cancelled" => false,
            _ => false,
        };
        anyhow::ensure!(
            allowed,
            "invalid task transition: {} -> {}",
            current.state,
            next
        );
        let updated_at = now();
        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute(
            "UPDATE tasks SET state=?2, updated_at=?3 WHERE id=?1 AND state=?4",
            params![id, next, updated_at, current.state],
        )?;
        anyhow::ensure!(changed == 1, "task changed concurrently; reload and retry");
        record_task_event_on(
            &tx,
            id,
            "task.state_changed",
            &serde_json::json!({"from": current.state, "to": next}).to_string(),
        )?;
        tx.commit()?;
        self.get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task disappeared after state update"))
    }

    pub fn cancel_task(&self, id: &str) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute(
            "UPDATE tasks SET state='cancelled',updated_at=?2 WHERE id=?1 AND state IN ('queued','paused')",
            params![id, now()],
        )?;
        if changed > 0 {
            record_task_event_on(&tx, id, "task.cancelled", "{}")?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    /// Persist a user-triggered, read-only agent attempt and atomically claim its task.
    pub fn start_task_run(&self, task_id: &str, prompt: &str) -> Result<TaskRun> {
        let prompt = prompt.trim();
        anyhow::ensure!(!prompt.is_empty(), "task run prompt cannot be empty");
        anyhow::ensure!(
            prompt.len() <= 32 * 1024,
            "task run prompt exceeds the 32 KiB limit"
        );
        let run = TaskRun {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: task_id.to_owned(),
            prompt: prompt.to_owned(),
            status: "running".to_owned(),
            result: None,
            error: None,
            usage: None,
            started_at: now(),
            finished_at: None,
        };
        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute(
            "UPDATE tasks SET state='running',updated_at=?2 WHERE id=?1 AND state='queued'",
            params![task_id, run.started_at],
        )?;
        anyhow::ensure!(
            changed == 1,
            "task not found or not queued; only queued tasks can be run"
        );
        tx.execute(
            "INSERT INTO task_runs(id,task_id,prompt,status,started_at) VALUES(?1,?2,?3,'running',?4)",
            params![run.id, run.task_id, run.prompt, run.started_at],
        )?;
        record_task_event_on(
            &tx,
            task_id,
            "task.run_started",
            &serde_json::json!({"run_id":run.id}).to_string(),
        )?;
        tx.commit()?;
        Ok(run)
    }

    /// Recover attempts left running by a process that exited unexpectedly.
    /// Recovery is idempotent and commits task/run state with its audit events.
    pub fn recover_interrupted_runs(&self) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let interrupted: Vec<(String, String)> = {
            let mut stmt = tx.prepare(
                "SELECT id,task_id FROM task_runs WHERE status='running' ORDER BY started_at",
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut recovered = 0;
        for (run_id, task_id) in interrupted {
            let finished_at = now();
            let error = "interrupted by runtime shutdown or process crash; explicit retry is required";
            let changed = tx.execute(
                "UPDATE task_runs SET status='failed',error=?2,finished_at=?3
                 WHERE id=?1 AND status='running'",
                params![run_id, error, finished_at],
            )?;
            if changed == 0 {
                continue;
            }
            tx.execute(
                "UPDATE tasks SET state='failed',updated_at=?2
                 WHERE id=?1 AND state='running'",
                params![task_id, finished_at],
            )?;
            record_task_event_on(
                &tx,
                &task_id,
                "task.run_interrupted",
                &serde_json::json!({"run_id":run_id,"reason":"runtime_recovery"}).to_string(),
            )?;
            record_task_event_on(
                &tx,
                &task_id,
                "task.state_changed",
                &serde_json::json!({"from":"running","to":"failed","reason":"runtime_recovery"}).to_string(),
            )?;
            recovered += 1;
        }
        tx.commit()?;
        Ok(recovered)
    }

    /// Record the provider outcome. Model success moves the task to paused for
    /// human review; it is never treated as proof that the objective is complete.
    pub fn finish_task_run(
        &self,
        run_id: &str,
        succeeded: bool,
        result: Option<&str>,
        error: Option<&str>,
        usage: Option<&Value>,
    ) -> Result<TaskRun> {
        if succeeded {
            anyhow::ensure!(
                result.is_some(),
                "successful task runs must persist a result"
            );
        } else {
            anyhow::ensure!(
                error.is_some_and(|value| !value.trim().is_empty()),
                "failed task runs must persist an error"
            );
        }
        if let Some(text) = result {
            anyhow::ensure!(
                text.len() <= 2 * 1024 * 1024,
                "task result exceeds the 2 MiB limit"
            );
        }
        if let Some(text) = error {
            anyhow::ensure!(text.len() <= 8192, "task error exceeds the 8 KiB limit");
        }
        let next_run_state = if succeeded { "succeeded" } else { "failed" };
        let next_task_state = if succeeded { "paused" } else { "failed" };
        let finished_at = now();
        let usage_json = usage.map(serde_json::to_string).transpose()?;
        if let Some(serialized) = usage_json.as_deref() {
            anyhow::ensure!(
                serialized.len() <= 4096,
                "task usage snapshot exceeds 4 KiB"
            );
        }

        let tx = self.conn.unchecked_transaction()?;
        let task_id: String = tx
            .query_row(
                "SELECT task_id FROM task_runs WHERE id=?1 AND status='running'",
                [run_id],
                |row| row.get(0),
            )
            .context("task run not found or already finished")?;
        let changed = tx.execute(
            "UPDATE task_runs SET status=?2,result=?3,error=?4,usage_json=?5,finished_at=?6
             WHERE id=?1 AND status='running'",
            params![
                run_id,
                next_run_state,
                result,
                error,
                usage_json,
                finished_at
            ],
        )?;
        anyhow::ensure!(
            changed == 1,
            "task run changed concurrently; reload and retry"
        );
        let task_changed = tx.execute(
            "UPDATE tasks SET state=?2,updated_at=?3 WHERE id=?1 AND state='running'",
            params![task_id, next_task_state, finished_at],
        )?;
        anyhow::ensure!(
            task_changed == 1,
            "task is no longer running; run result was not applied"
        );
        record_task_event_on(
            &tx,
            &task_id,
            "task.run_finished",
            &serde_json::json!({
                "run_id":run_id,
                "status":next_run_state,
                "result_bytes":result.map(str::len),
                "has_error":error.is_some()
            })
            .to_string(),
        )?;
        record_task_event_on(
            &tx,
            &task_id,
            "task.state_changed",
            &serde_json::json!({"from":"running","to":next_task_state,"reason":"agent_run_finished"}).to_string(),
        )?;
        tx.commit()?;
        self.get_task_run(run_id)?
            .ok_or_else(|| anyhow::anyhow!("task run disappeared after completion"))
    }

    pub fn latest_task_run(&self, task_id: &str) -> Result<Option<TaskRun>> {
        let id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM task_runs WHERE task_id=?1 ORDER BY started_at DESC LIMIT 1",
                [task_id],
                |row| row.get(0),
            )
            .optional()?;
        match id {
            Some(id) => self.get_task_run(&id),
            None => Ok(None),
        }
    }

    fn get_task_run(&self, run_id: &str) -> Result<Option<TaskRun>> {
        let row = self
            .conn
            .query_row(
                "SELECT id,task_id,prompt,status,result,error,usage_json,started_at,finished_at
                 FROM task_runs WHERE id=?1",
                [run_id],
                |row| {
                    let usage_json: Option<String> = row.get(6)?;
                    let usage = usage_json
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                6,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(TaskRun {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        prompt: row.get(2)?,
                        status: row.get(3)?,
                        result: row.get(4)?,
                        error: row.get(5)?,
                        usage,
                        started_at: row.get(7)?,
                        finished_at: row.get(8)?,
                    })
                },
            )
            .optional()
            .context("read task run")?;
        Ok(row)
    }

    pub fn grant_capability(&self, capability: &str, note: &str) -> Result<CapabilityGrant> {
        let capability = validate_key(capability, "capability")?;
        let note = note.trim();
        anyhow::ensure!(!note.is_empty(), "grant note cannot be empty");
        anyhow::ensure!(note.len() <= 1000, "grant note must be 1000 bytes or fewer");
        let granted_at = now();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO capability_grants(capability,note,granted_at) VALUES(?1,?2,?3)
             ON CONFLICT(capability) DO UPDATE SET note=excluded.note, granted_at=excluded.granted_at",
            params![capability, note, granted_at],
        )?;
        record_system_event_on(
            &tx,
            "capability.granted",
            &serde_json::json!({"capability":capability,"note":note}).to_string(),
        )?;
        tx.commit()?;
        Ok(CapabilityGrant {
            capability,
            note: note.to_owned(),
            granted_at,
        })
    }

    pub fn revoke_capability(&self, capability: &str) -> Result<bool> {
        let capability = validate_key(capability, "capability")?;
        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute(
            "DELETE FROM capability_grants WHERE capability=?1",
            [capability.as_str()],
        )?;
        if changed > 0 {
            record_system_event_on(
                &tx,
                "capability.revoked",
                &serde_json::json!({"capability":capability}).to_string(),
            )?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    pub fn has_capability(&self, capability: &str) -> Result<bool> {
        let capability = validate_key(capability, "capability")?;
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM capability_grants WHERE capability=?1)",
            [capability],
            |row| row.get(0),
        )?)
    }

    pub fn list_capabilities(&self) -> Result<Vec<CapabilityGrant>> {
        let mut stmt = self.conn.prepare(
            "SELECT capability,note,granted_at FROM capability_grants ORDER BY capability",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CapabilityGrant {
                capability: row.get(0)?,
                note: row.get(1)?,
                granted_at: row.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("read capability grants")
    }

    pub fn create_approval(&self, action: &str, reason: &str) -> Result<Approval> {
        let action = validate_key(action, "approval action")?;
        let reason = reason.trim();
        anyhow::ensure!(!reason.is_empty(), "approval reason cannot be empty");
        anyhow::ensure!(
            reason.len() <= 2000,
            "approval reason must be 2000 bytes or fewer"
        );
        let approval = Approval {
            id: uuid::Uuid::new_v4().to_string(),
            action,
            reason: reason.to_owned(),
            state: "pending".to_owned(),
            created_at: now(),
        };
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO approvals(id,action,reason,state,created_at) VALUES(?1,?2,?3,'pending',?4)",
            params![approval.id, approval.action, approval.reason, approval.created_at],
        )?;
        record_system_event_on(
            &tx,
            "approval.requested",
            &serde_json::to_string(&approval)?,
        )?;
        tx.commit()?;
        Ok(approval)
    }

    pub fn resolve_approval(&self, id: &str, approved: bool) -> Result<bool> {
        let next = if approved { "approved" } else { "denied" };
        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute(
            "UPDATE approvals SET state=?2 WHERE id=?1 AND state='pending'",
            params![id, next],
        )?;
        if changed > 0 {
            record_system_event_on(
                &tx,
                "approval.resolved",
                &serde_json::json!({"id":id,"state":next}).to_string(),
            )?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    pub fn list_approvals(&self) -> Result<Vec<Approval>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,action,reason,state,created_at FROM approvals ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Approval {
                id: row.get(0)?,
                action: row.get(1)?,
                reason: row.get(2)?,
                state: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("read approval requests")
    }

    pub fn approval_is_approved(&self, id: &str, action: &str) -> Result<bool> {
        let action = validate_key(action, "approval action")?;
        Ok(self.conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM approvals a
                WHERE a.id=?1 AND a.action=?2 AND a.state='approved'
                  AND NOT EXISTS(
                    SELECT 1 FROM approval_uses u WHERE u.approval_id=a.id
                  )
             )",
            params![id, action],
            |row| row.get(0),
        )?)
    }

    /// Atomically consume an approved, action-scoped approval exactly once.
    ///
    /// Reserved for the future tool executor; deliberately not exposed as a
    /// standalone CLI operation because that would consume approval without
    /// performing the approved action.
    #[allow(dead_code)]
    pub fn consume_approval(&self, id: &str, action: &str) -> Result<bool> {
        let action = validate_key(action, "approval action")?;
        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute(
            "INSERT INTO approval_uses(approval_id,consumed_at)
             SELECT id,?3 FROM approvals
             WHERE id=?1 AND action=?2 AND state='approved'
             ON CONFLICT(approval_id) DO NOTHING",
            params![id, action, now()],
        )?;
        if changed > 0 {
            record_system_event_on(
                &tx,
                "approval.consumed",
                &serde_json::json!({"id":id,"action":action}).to_string(),
            )?;
        }
        tx.commit()?;
        Ok(changed == 1)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let key = validate_key(key, "setting key")?;
        self.conn
            .query_row(
                "SELECT value FROM app_settings WHERE key=?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .context("read application setting")
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let key = validate_key(key, "setting key")?;
        anyhow::ensure!(
            value.len() <= 4096,
            "setting value must be 4096 bytes or fewer"
        );
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO app_settings(key,value,updated_at) VALUES(?1,?2,?3)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
            params![key, value, now()],
        )?;
        record_system_event_on(
            &tx,
            "setting.updated",
            &serde_json::json!({"key":key}).to_string(),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove_setting(&self, key: &str) -> Result<bool> {
        let key = validate_key(key, "setting key")?;
        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute("DELETE FROM app_settings WHERE key=?1", [key.as_str()])?;
        if changed > 0 {
            record_system_event_on(
                &tx,
                "setting.removed",
                &serde_json::json!({"key":key}).to_string(),
            )?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    pub fn record_audit_event(&self, event_type: &str, payload: &str) -> Result<()> {
        let event_type = validate_key(event_type, "event type")?;
        anyhow::ensure!(
            payload.len() <= 4096,
            "audit payload must be 4096 bytes or fewer"
        );
        let tx = self.conn.unchecked_transaction()?;
        record_system_event_on(&tx, &event_type, payload)?;
        tx.commit()?;
        Ok(())
    }

    pub fn event_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))?)
    }
}

fn record_task_event_on(conn: &Connection, task_id: &str, kind: &str, payload: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO events(task_id,event_type,payload,created_at) VALUES(?1,?2,?3,?4)",
        params![task_id, kind, payload, now()],
    )?;
    Ok(())
}

fn record_system_event_on(conn: &Connection, kind: &str, payload: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO events(task_id,event_type,payload,created_at) VALUES(NULL,?1,?2,?3)",
        params![kind, payload, now()],
    )?;
    Ok(())
}

fn validate_key(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    anyhow::ensure!(!value.is_empty(), "{label} cannot be empty");
    anyhow::ensure!(value.len() <= 128, "{label} must be 128 bytes or fewer");
    anyhow::ensure!(
        value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "._:-".contains(ch)),
        "{label} may contain only ASCII letters, numbers, dot, underscore, colon and hyphen"
    );
    Ok(value.to_owned())
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
    fn task_mutations_roll_back_when_event_recording_fails() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TRIGGER reject_task_created
                 BEFORE INSERT ON events
                 WHEN NEW.event_type='task.created'
                 BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
            )
            .unwrap();
        assert!(store
            .create_task("Atomic create", "must roll back")
            .is_err());
        assert!(store.list_tasks().unwrap().is_empty());

        store
            .conn
            .execute_batch("DROP TRIGGER reject_task_created;")
            .unwrap();
        let task = store
            .create_task("Atomic transition", "must remain queued")
            .unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TRIGGER reject_task_mutations
                 BEFORE INSERT ON events
                 WHEN NEW.event_type IN ('task.state_changed', 'task.cancelled')
                 BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
            )
            .unwrap();
        assert!(store.transition_task(&task.id, "running").is_err());
        assert!(store.cancel_task(&task.id).is_err());
        assert_eq!(store.get_task(&task.id).unwrap().unwrap().state, "queued");
    }

    #[test]
    fn task_runs_persist_results_and_wait_for_owner_acceptance() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let task = store
            .create_task("Inspect workspace", "Summarize the project structure")
            .unwrap();
        let run = store
            .start_task_run(&task.id, "Inspect the project read-only")
            .unwrap();
        assert_eq!(store.get_task(&task.id).unwrap().unwrap().state, "running");
        assert!(!store.cancel_task(&task.id).unwrap());

        let usage = serde_json::json!({"isQuotaExceeded":false,"session":{"total_credits":12}});
        let finished = store
            .finish_task_run(
                &run.id,
                true,
                Some("Project contains a Rust CLI and a Node worker."),
                None,
                Some(&usage),
            )
            .unwrap();
        assert_eq!(finished.status, "succeeded");
        assert_eq!(finished.usage.as_ref().unwrap()["isQuotaExceeded"], false);
        assert_eq!(store.get_task(&task.id).unwrap().unwrap().state, "paused");
        assert!(store
            .finish_task_run(&run.id, true, Some("duplicate"), None, None)
            .is_err());

        let latest = store.latest_task_run(&task.id).unwrap().unwrap();
        assert_eq!(
            latest.result.as_deref(),
            Some("Project contains a Rust CLI and a Node worker.")
        );
        assert_eq!(
            store.transition_task(&task.id, "completed").unwrap().state,
            "completed"
        );
    }

    #[test]
    fn task_run_failures_mark_task_failed_and_can_be_retried_explicitly() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let task = store
            .create_task("Inspect workspace", "Read a project file")
            .unwrap();
        let run = store
            .start_task_run(&task.id, "Read a project file")
            .unwrap();
        let finished = store
            .finish_task_run(&run.id, false, None, Some("Qoder quota unavailable"), None)
            .unwrap();
        assert_eq!(finished.status, "failed");
        assert_eq!(finished.error.as_deref(), Some("Qoder quota unavailable"));
        assert_eq!(store.get_task(&task.id).unwrap().unwrap().state, "failed");
        assert_eq!(
            store.transition_task(&task.id, "queued").unwrap().state,
            "queued"
        );
    }

    #[test]
    fn task_run_start_and_finish_are_atomic_with_the_event_log() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let task = store
            .create_task("Atomic run", "Test transaction boundaries")
            .unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TRIGGER reject_task_run_started
                 BEFORE INSERT ON events
                 WHEN NEW.event_type='task.run_started'
                 BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
            )
            .unwrap();
        assert!(store.start_task_run(&task.id, "Run prompt").is_err());
        assert_eq!(store.get_task(&task.id).unwrap().unwrap().state, "queued");
        let run_count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM task_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(run_count, 0);

        store
            .conn
            .execute_batch("DROP TRIGGER reject_task_run_started;")
            .unwrap();
        let run = store.start_task_run(&task.id, "Run prompt").unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TRIGGER reject_task_run_finished
                 BEFORE INSERT ON events
                 WHEN NEW.event_type='task.run_finished'
                 BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
            )
            .unwrap();
        assert!(store
            .finish_task_run(&run.id, true, Some("result"), None, None)
            .is_err());
        assert_eq!(store.get_task(&task.id).unwrap().unwrap().state, "running");
        let status: String = store
            .conn
            .query_row(
                "SELECT status FROM task_runs WHERE id=?1",
                [&run.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "running");
    }

    #[test]
    fn recovers_interrupted_attempts_once_and_requires_explicit_retry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fairy.sqlite3");
        let task_id;
        {
            let store = Store::open(&path).unwrap();
            let task = store.create_task("Crash recovery", "Recover an interrupted worker").unwrap();
            task_id = task.id.clone();
            store.start_task_run(&task_id, "Inspect safely").unwrap();
            assert_eq!(store.get_task(&task_id).unwrap().unwrap().state, "running");
        }

        let recovered = Store::open(&path).unwrap();
        assert_eq!(recovered.recover_interrupted_runs().unwrap(), 1);
        assert_eq!(recovered.recover_interrupted_runs().unwrap(), 0);
        assert_eq!(recovered.get_task(&task_id).unwrap().unwrap().state, "failed");
        let run = recovered.latest_task_run(&task_id).unwrap().unwrap();
        assert_eq!(run.status, "failed");
        assert!(run.error.unwrap().contains("explicit retry"));
        assert_eq!(recovered.event_count().unwrap(), 4);
        let retryable = recovered.transition_task(&task_id, "queued").unwrap();
        assert_eq!(retryable.state, "queued");
    }

    #[test]
    fn enforces_task_state_machine_and_records_transitions() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let task = store
            .create_task("State machine", "Verify legal transitions")
            .unwrap();

        let running = store.transition_task(&task.id, "running");
        assert!(running.is_err());
        let paused = store.transition_task(&task.id, "paused").unwrap();
        assert_eq!(paused.state, "paused");
        let completed = store.transition_task(&task.id, "completed").unwrap();
        assert_eq!(completed.state, "completed");
        assert!(store.transition_task(&task.id, "queued").is_err());
        assert_eq!(store.event_count().unwrap(), 3);
    }

    #[test]
    fn grants_are_explicit_reversible_and_audited() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        assert!(!store.has_capability("filesystem.read").unwrap());
        store
            .grant_capability("filesystem.read", "Owner approved read access")
            .unwrap();
        assert!(store.has_capability("filesystem.read").unwrap());
        assert_eq!(store.list_capabilities().unwrap().len(), 1);
        assert!(store.revoke_capability("filesystem.read").unwrap());
        assert!(!store.has_capability("filesystem.read").unwrap());
        assert_eq!(store.event_count().unwrap(), 2);
    }

    #[test]
    fn approvals_are_action_scoped_and_consumed_once() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let approval = store
            .create_approval("filesystem.delete", "Delete generated build output")
            .unwrap();
        assert!(!store
            .approval_is_approved(&approval.id, "filesystem.delete")
            .unwrap());
        assert!(store.resolve_approval(&approval.id, true).unwrap());
        assert!(!store.resolve_approval(&approval.id, false).unwrap());
        assert!(!store
            .approval_is_approved(&approval.id, "process.kill")
            .unwrap());
        assert!(store
            .approval_is_approved(&approval.id, "filesystem.delete")
            .unwrap());
        assert!(store
            .consume_approval(&approval.id, "filesystem.delete")
            .unwrap());
        assert!(!store
            .consume_approval(&approval.id, "filesystem.delete")
            .unwrap());
        assert!(!store
            .approval_is_approved(&approval.id, "filesystem.delete")
            .unwrap());
    }

    #[test]
    fn rejects_wildcard_and_invalid_capability_names() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        assert!(store.grant_capability("*", "too broad").is_err());
        assert!(store
            .grant_capability("shell;admin", "invalid syntax")
            .is_err());
    }

    #[test]
    fn upgrades_existing_schema_v1_database_without_losing_tasks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.sqlite3");
        {
            let legacy = Connection::open(&path).unwrap();
            legacy
                .execute_batch(
                    "CREATE TABLE schema_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                     INSERT INTO schema_meta(key,value) VALUES('schema_version','1');
                     CREATE TABLE tasks (
                        id TEXT PRIMARY KEY,
                        title TEXT NOT NULL,
                        objective TEXT NOT NULL,
                        state TEXT NOT NULL CHECK(state IN ('queued','running','paused','completed','failed','cancelled')),
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL
                     );
                     CREATE TABLE events (
                        seq INTEGER PRIMARY KEY AUTOINCREMENT,
                        task_id TEXT,
                        event_type TEXT NOT NULL,
                        payload TEXT NOT NULL,
                        created_at INTEGER NOT NULL,
                        FOREIGN KEY(task_id) REFERENCES tasks(id)
                     );
                     CREATE TABLE approvals (
                        id TEXT PRIMARY KEY,
                        action TEXT NOT NULL,
                        reason TEXT NOT NULL,
                        state TEXT NOT NULL CHECK(state IN ('pending','approved','denied')),
                        created_at INTEGER NOT NULL
                     );
                     INSERT INTO tasks(id,title,objective,state,created_at,updated_at)
                     VALUES('legacy-task','Old task','Keep this task','queued',1,1);",
                )
                .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let version: String = store
            .conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, "4");
        assert_eq!(
            store.get_task("legacy-task").unwrap().unwrap().title,
            "Old task"
        );
        let task_run_table_exists: bool = store
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_runs')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(task_run_table_exists);
        store
            .grant_capability("filesystem.read", "Migration test")
            .unwrap();
        assert!(store.has_capability("filesystem.read").unwrap());
        assert!(store
            .create_approval("filesystem.delete", "Migration test")
            .is_ok());
        store
            .set_setting("workspace_root", "/tmp/workspace")
            .unwrap();
        assert_eq!(
            store.get_setting("workspace_root").unwrap().as_deref(),
            Some("/tmp/workspace")
        );
    }

    #[test]
    fn capability_grant_rolls_back_when_audit_insert_fails() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TRIGGER reject_capability_audit
             BEFORE INSERT ON events
             WHEN NEW.event_type='capability.granted'
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
            )
            .unwrap();
        assert!(store
            .grant_capability("filesystem.read", "must be audited")
            .is_err());
        assert!(!store.has_capability("filesystem.read").unwrap());
    }

    #[test]
    fn approval_creation_rolls_back_when_audit_insert_fails() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TRIGGER reject_approval_audit
             BEFORE INSERT ON events
             WHEN NEW.event_type='approval.requested'
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
            )
            .unwrap();
        assert!(store
            .create_approval("filesystem.delete", "must be audited")
            .is_err());
        assert!(store.list_approvals().unwrap().is_empty());
    }

    #[test]
    fn setting_updates_are_audited_and_removable() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        assert_eq!(store.get_setting("workspace_root").unwrap(), None);
        store.set_setting("workspace_root", "/workspace").unwrap();
        assert_eq!(
            store.get_setting("workspace_root").unwrap().as_deref(),
            Some("/workspace")
        );
        assert!(store.remove_setting("workspace_root").unwrap());
        assert!(!store.remove_setting("workspace_root").unwrap());
        assert_eq!(store.event_count().unwrap(), 2);
    }

    #[test]
    fn rejects_empty_task_fields() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        assert!(store.create_task(" ", "objective").is_err());
        assert!(store.create_task("title", " ").is_err());
    }
}
