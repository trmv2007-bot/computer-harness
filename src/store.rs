use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
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
             INSERT OR IGNORE INTO schema_meta(key,value) VALUES('schema_version','3');
             UPDATE schema_meta SET value='3'
                WHERE key='schema_version' AND CAST(value AS INTEGER)<3;
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
        record_task_event_on(&tx, &task.id, "task.created", &serde_json::to_string(&task)?)?;
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
            "queued" => matches!(next, "running" | "paused" | "failed" | "cancelled"),
            "running" => matches!(next, "paused" | "completed" | "failed" | "cancelled"),
            "paused" => matches!(next, "queued" | "failed" | "cancelled"),
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
            "UPDATE tasks SET state='cancelled',updated_at=?2 WHERE id=?1 AND state IN ('queued','running','paused')",
            params![id, now()],
        )?;
        if changed > 0 {
            record_task_event_on(&tx, id, "task.cancelled", "{}")?;
        }
        tx.commit()?;
        Ok(changed > 0)
    }

    pub fn record_event(&self, task_id: &str, kind: &str, payload: &str) -> Result<()> {
        record_task_event_on(&self.conn, task_id, kind, payload)
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
        store.conn.execute_batch(
            "CREATE TRIGGER reject_task_created
             BEFORE INSERT ON events
             WHEN NEW.event_type='task.created'
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;"
        ).unwrap();
        assert!(store.create_task("Atomic create", "must roll back").is_err());
        assert!(store.list_tasks().unwrap().is_empty());

        store.conn.execute_batch("DROP TRIGGER reject_task_created;").unwrap();
        let task = store.create_task("Atomic transition", "must remain queued").unwrap();
        store.conn.execute_batch(
            "CREATE TRIGGER reject_task_state
             BEFORE INSERT ON events
             WHEN NEW.event_type='task.state_changed'
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;"
        ).unwrap();
        assert!(store.transition_task(&task.id, "running").is_err());
        assert_eq!(store.get_task(&task.id).unwrap().unwrap().state, "queued");
        assert!(store.cancel_task(&task.id).is_ok());
    }

    #[test]
    fn enforces_task_state_machine_and_records_transitions() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let task = store
            .create_task("State machine", "Verify legal transitions")
            .unwrap();
        let running = store.transition_task(&task.id, "running").unwrap();
        assert_eq!(running.state, "running");
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
        assert_eq!(version, "3");
        assert_eq!(
            store.get_task("legacy-task").unwrap().unwrap().title,
            "Old task"
        );
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
