//! Agent session lifecycle (T012) and the global single-session lock (T020).

use std::sync::{Arc, Mutex};

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::storage::{now, Database};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Idle,
    Running,
    WaitingApproval,
    Interrupted,
    Complete,
}

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionStatus::Idle => "idle",
            SessionStatus::Running => "running",
            SessionStatus::WaitingApproval => "waiting_approval",
            SessionStatus::Interrupted => "interrupted",
            SessionStatus::Complete => "complete",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "idle" => Ok(SessionStatus::Idle),
            "running" => Ok(SessionStatus::Running),
            "waiting_approval" => Ok(SessionStatus::WaitingApproval),
            "interrupted" => Ok(SessionStatus::Interrupted),
            "complete" => Ok(SessionStatus::Complete),
            other => Err(Error::Validation(format!("unknown session status {other}"))),
        }
    }

    /// Terminal states never transition again.
    pub fn is_terminal(self) -> bool {
        matches!(self, SessionStatus::Interrupted | SessionStatus::Complete)
    }

    /// Whether the session occupies the single global agent slot (FR-022).
    pub fn is_active(self) -> bool {
        matches!(
            self,
            SessionStatus::Running | SessionStatus::WaitingApproval
        )
    }

    /// Allowed transitions: idle -> running -> waiting_approval -> running,
    /// with interrupted/complete reachable from any non-terminal state.
    pub fn can_transition_to(self, next: SessionStatus) -> bool {
        use SessionStatus::*;
        match (self, next) {
            (from, _) if from.is_terminal() => false,
            (_, Interrupted) | (_, Complete) => true,
            (Idle, Running) => true,
            (Running, WaitingApproval) => true,
            (WaitingApproval, Running) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: i64,
    pub project_id: i64,
    pub provider_id: i64,
    pub status: SessionStatus,
    pub prompt_text: String,
    pub origin_device_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

impl AgentSession {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let status: String = row.get("status")?;
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            provider_id: row.get("provider_id")?,
            status: SessionStatus::parse(&status).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    err.into(),
                )
            })?,
            prompt_text: row.get("prompt_text")?,
            origin_device_id: row.get("origin_device_id")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            completed_at: row.get("completed_at")?,
        })
    }
}

const SELECT: &str = "SELECT id, project_id, provider_id, status, prompt_text, origin_device_id, \
                      created_at, updated_at, completed_at FROM agent_sessions";

pub struct SessionRepository<'a> {
    db: &'a Database,
}

impl<'a> SessionRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Create a session in the `idle` state for an available project.
    pub fn create(
        &self,
        project_id: i64,
        provider_id: i64,
        prompt_text: &str,
        origin_device_id: Option<i64>,
    ) -> Result<AgentSession> {
        if prompt_text.trim().is_empty() {
            return Err(Error::Validation("prompt must not be empty".into()));
        }
        let project = crate::storage::projects::ProjectRepository::new(self.db).get(project_id)?;
        if !project.available {
            return Err(Error::Validation(format!(
                "project {} is unavailable on disk",
                project.name
            )));
        }
        // Ensures the provider exists before a session references it.
        crate::agent::provider::ModelProviderRepository::new(self.db).get(provider_id)?;

        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO agent_sessions (project_id, provider_id, status, prompt_text, \
             origin_device_id, created_at, updated_at) VALUES (?1, ?2, 'idle', ?3, ?4, ?5, ?5)",
            params![
                project_id,
                provider_id,
                prompt_text,
                origin_device_id,
                now()
            ],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get(id)
    }

    pub fn get(&self, id: i64) -> Result<AgentSession> {
        self.db
            .conn()
            .query_row(
                &format!("{SELECT} WHERE id = ?1"),
                [id],
                AgentSession::from_row,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("session {id}")),
                other => other.into(),
            })
    }

    pub fn list(&self) -> Result<Vec<AgentSession>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY created_at DESC"))?;
        let sessions = stmt
            .query_map([], AgentSession::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(sessions)
    }

    /// The session currently occupying the global agent slot, if any.
    pub fn active(&self) -> Result<Option<AgentSession>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE status IN ('running', 'waiting_approval') ORDER BY updated_at LIMIT 1"
        ))?;
        let mut rows = stmt.query_map([], AgentSession::from_row)?;
        rows.next().transpose().map_err(Into::into)
    }

    /// Move a session to `next`, rejecting illegal transitions and refusing a
    /// second concurrently active session (FR-022).
    pub fn update_status(&self, id: i64, next: SessionStatus) -> Result<AgentSession> {
        let session = self.get(id)?;
        if session.status == next {
            return Ok(session);
        }
        if !session.status.can_transition_to(next) {
            return Err(Error::Validation(format!(
                "cannot move session {id} from {} to {}",
                session.status.as_str(),
                next.as_str()
            )));
        }
        if next.is_active() {
            if let Some(active) = self.active()? {
                if active.id != id {
                    let project = crate::storage::projects::ProjectRepository::new(self.db)
                        .get(active.project_id)?;
                    return Err(Error::SessionBusy {
                        project: project.name,
                    });
                }
            }
        }

        let completed_at = next.is_terminal().then(now);
        self.db.conn().execute(
            "UPDATE agent_sessions SET status = ?2, updated_at = ?3, completed_at = \
             COALESCE(?4, completed_at) WHERE id = ?1",
            params![id, next.as_str(), now(), completed_at],
        )?;
        self.get(id)
    }

    /// Delete a session; transcript entries, approvals, and connections cascade
    /// (FR-007g).
    pub fn delete(&self, id: i64) -> Result<()> {
        let changed = self
            .db
            .conn()
            .execute("DELETE FROM agent_sessions WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(Error::NotFound(format!("session {id}")));
        }
        Ok(())
    }
}

/// Process-wide guard enforcing at most one running session across all
/// projects (FR-022). Held for the lifetime of an agent run.
#[derive(Debug, Clone, Default)]
pub struct SessionLock {
    holder: Arc<Mutex<Option<i64>>>,
}

impl SessionLock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the global slot for `session_id`.
    ///
    /// Returns [`Error::SessionBusy`] with the busy project name if another
    /// session already holds it.
    pub fn acquire(
        &self,
        session_id: i64,
        busy_project_name: impl Fn() -> String,
    ) -> Result<SessionGuard> {
        let mut holder = self.holder.lock().expect("session lock poisoned");
        match *holder {
            Some(_) => Err(Error::SessionBusy {
                project: busy_project_name(),
            }),
            None => {
                *holder = Some(session_id);
                Ok(SessionGuard {
                    lock: self.clone(),
                    session_id,
                })
            }
        }
    }

    /// Id of the session currently holding the slot.
    pub fn current(&self) -> Option<i64> {
        *self.holder.lock().expect("session lock poisoned")
    }

    fn release(&self, session_id: i64) {
        let mut holder = self.holder.lock().expect("session lock poisoned");
        if *holder == Some(session_id) {
            *holder = None;
        }
    }
}

/// Releases the global session slot when dropped.
#[derive(Debug)]
pub struct SessionGuard {
    lock: SessionLock,
    session_id: i64,
}

impl SessionGuard {
    pub fn session_id(&self) -> i64 {
        self.session_id
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.lock.release(self.session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::seed_project_and_provider;

    #[test]
    fn enforces_status_transitions() {
        use SessionStatus::*;
        assert!(Idle.can_transition_to(Running));
        assert!(Running.can_transition_to(WaitingApproval));
        assert!(WaitingApproval.can_transition_to(Running));
        assert!(Running.can_transition_to(Complete));
        assert!(!Idle.can_transition_to(WaitingApproval));
        assert!(!Complete.can_transition_to(Running));
        assert!(!Interrupted.can_transition_to(Complete));
    }

    #[test]
    fn creates_and_advances_a_session() {
        let db = Database::open_in_memory().unwrap();
        let (_dir, project_id, provider_id) = seed_project_and_provider(&db);
        let repo = SessionRepository::new(&db);

        let session = repo
            .create(project_id, provider_id, "build it", None)
            .unwrap();
        assert_eq!(session.status, SessionStatus::Idle);

        let running = repo
            .update_status(session.id, SessionStatus::Running)
            .unwrap();
        assert_eq!(running.status, SessionStatus::Running);
        assert_eq!(repo.active().unwrap().unwrap().id, session.id);

        let complete = repo
            .update_status(session.id, SessionStatus::Complete)
            .unwrap();
        assert!(complete.completed_at.is_some());
        assert!(repo.active().unwrap().is_none());

        assert!(matches!(
            repo.update_status(session.id, SessionStatus::Running),
            Err(Error::Validation(_))
        ));
        assert!(matches!(
            repo.create(project_id, provider_id, "   ", None),
            Err(Error::Validation(_))
        ));
    }

    #[test]
    fn refuses_a_second_active_session() {
        let db = Database::open_in_memory().unwrap();
        let (_dir, project_id, provider_id) = seed_project_and_provider(&db);
        let repo = SessionRepository::new(&db);

        let first = repo.create(project_id, provider_id, "one", None).unwrap();
        let second = repo.create(project_id, provider_id, "two", None).unwrap();
        repo.update_status(first.id, SessionStatus::Running)
            .unwrap();

        assert!(matches!(
            repo.update_status(second.id, SessionStatus::Running),
            Err(Error::SessionBusy { .. })
        ));

        repo.update_status(first.id, SessionStatus::Complete)
            .unwrap();
        repo.update_status(second.id, SessionStatus::Running)
            .unwrap();
    }

    #[test]
    fn global_lock_admits_one_holder_and_releases_on_drop() {
        let lock = SessionLock::new();
        let guard = lock.acquire(1, || "demo".into()).unwrap();
        assert_eq!(lock.current(), Some(1));
        assert!(matches!(
            lock.acquire(2, || "demo".into()),
            Err(Error::SessionBusy { .. })
        ));
        // The same session cannot take the slot twice either.
        assert!(matches!(
            lock.acquire(1, || "demo".into()),
            Err(Error::SessionBusy { .. })
        ));
        drop(guard);
        assert_eq!(lock.current(), None);
        lock.acquire(2, || "demo".into()).unwrap();
    }
}
