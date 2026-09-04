//! Approval request storage with resolve-once semantics (T014).

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::storage::{now, Database};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalKind {
    FileChange,
    Command,
}

impl ApprovalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ApprovalKind::FileChange => "file_change",
            ApprovalKind::Command => "command",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "file_change" => Ok(ApprovalKind::FileChange),
            "command" => Ok(ApprovalKind::Command),
            other => Err(Error::Validation(format!("unknown request type {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approved,
    Denied,
}

impl Decision {
    pub fn as_str(self) -> &'static str {
        match self {
            Decision::Approved => "approved",
            Decision::Denied => "denied",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "approved" => Ok(Decision::Approved),
            "denied" => Ok(Decision::Denied),
            other => Err(Error::Validation(format!("unknown decision {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: i64,
    pub session_id: i64,
    pub request_type: ApprovalKind,
    /// Type-specific JSON payload (see data-model.md).
    pub details: serde_json::Value,
    pub expires_at: String,
    pub resolved: bool,
    pub resolved_by_device_id: Option<i64>,
    pub resolution: Option<Decision>,
    pub created_at: String,
}

impl ApprovalRequest {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let request_type: String = row.get("request_type")?;
        let details: String = row.get("details")?;
        let resolution: Option<String> = row.get("resolution")?;
        let convert = |err: Error| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, err.into())
        };
        Ok(Self {
            id: row.get("id")?,
            session_id: row.get("session_id")?,
            request_type: ApprovalKind::parse(&request_type).map_err(convert)?,
            details: serde_json::from_str(&details).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?,
            expires_at: row.get("expires_at")?,
            resolved: row.get("resolved")?,
            resolved_by_device_id: row.get("resolved_by_device_id")?,
            resolution: resolution
                .as_deref()
                .map(Decision::parse)
                .transpose()
                .map_err(convert)?,
            created_at: row.get("created_at")?,
        })
    }

    /// Whether the approval window has passed (FR-019).
    pub fn is_expired(&self, at: DateTime<Utc>) -> bool {
        match DateTime::parse_from_rfc3339(&self.expires_at) {
            Ok(expires) => at >= expires.with_timezone(&Utc),
            Err(_) => false,
        }
    }
}

const SELECT: &str = "SELECT id, session_id, request_type, details, expires_at, resolved, \
                      resolved_by_device_id, resolution, created_at FROM approval_requests";

pub struct ApprovalRepository<'a> {
    db: &'a Database,
}

impl<'a> ApprovalRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Create a pending approval request expiring after `timeout`.
    pub fn create(
        &self,
        session_id: i64,
        request_type: ApprovalKind,
        details: &serde_json::Value,
        timeout: std::time::Duration,
    ) -> Result<ApprovalRequest> {
        let timeout = Duration::from_std(timeout)
            .map_err(|_| Error::Validation("approval timeout out of range".into()))?;
        if timeout <= Duration::zero() {
            return Err(Error::Validation(
                "approval timeout must be positive".into(),
            ));
        }
        let expires_at = (Utc::now() + timeout).to_rfc3339();
        let serialized = serde_json::to_string(details)?;

        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO approval_requests (session_id, request_type, details, expires_at, \
             resolved, created_at) VALUES (?1, ?2, ?3, ?4, 0, ?5)",
            params![
                session_id,
                request_type.as_str(),
                serialized,
                expires_at,
                now()
            ],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get(id)
    }

    pub fn get(&self, id: i64) -> Result<ApprovalRequest> {
        self.db
            .conn()
            .query_row(
                &format!("{SELECT} WHERE id = ?1"),
                [id],
                ApprovalRequest::from_row,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    Error::NotFound(format!("approval request {id}"))
                }
                other => other.into(),
            })
    }

    /// Unresolved requests for a session.
    pub fn pending(&self, session_id: i64) -> Result<Vec<ApprovalRequest>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE session_id = ?1 AND resolved = 0 ORDER BY created_at"
        ))?;
        let requests = stmt
            .query_map([session_id], ApprovalRequest::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(requests)
    }

    /// Record a decision. The first response wins; later ones are rejected
    /// with [`Error::AlreadyResolved`] (FR-005a/FR-005b).
    pub fn resolve(
        &self,
        id: i64,
        decision: Decision,
        device_id: Option<i64>,
    ) -> Result<ApprovalRequest> {
        let conn = self.db.conn();
        let changed = conn.execute(
            "UPDATE approval_requests SET resolved = 1, resolution = ?2, resolved_by_device_id = \
             ?3 WHERE id = ?1 AND resolved = 0",
            params![id, decision.as_str(), device_id],
        )?;
        drop(conn);

        let request = self.get(id)?;
        if changed == 0 {
            return Err(Error::AlreadyResolved {
                resolved_by: self.resolver_name(&request)?,
            });
        }
        Ok(request)
    }

    /// Auto-deny every unresolved request whose window has passed (FR-019).
    /// Returns the requests that were denied.
    pub fn expire_overdue(&self) -> Result<Vec<ApprovalRequest>> {
        let cutoff = Utc::now().to_rfc3339();
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE resolved = 0 AND expires_at <= ?1 ORDER BY expires_at"
        ))?;
        let overdue = stmt
            .query_map([&cutoff], ApprovalRequest::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        drop(conn);

        let mut denied = Vec::with_capacity(overdue.len());
        for request in overdue {
            match self.resolve(request.id, Decision::Denied, None) {
                Ok(resolved) => {
                    tracing::info!(request = resolved.id, "approval request expired, denied");
                    denied.push(resolved);
                }
                // Another device answered in the meantime; its decision wins.
                Err(Error::AlreadyResolved { .. }) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(denied)
    }

    /// Display name of whoever resolved a request ("desktop" when the decision
    /// came from the desktop IDE, "timeout" when it expired).
    fn resolver_name(&self, request: &ApprovalRequest) -> Result<String> {
        match request.resolved_by_device_id {
            Some(device_id) => Ok(
                crate::storage::pairing::PairedDeviceRepository::new(self.db)
                    .get(device_id)?
                    .device_name,
            ),
            None if request.resolved => Ok("desktop".to_string()),
            None => Ok("nobody".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::pairing::PairedDeviceRepository;
    use crate::test_support::seed_session;
    use std::time::Duration as StdDuration;

    fn details() -> serde_json::Value {
        serde_json::json!({ "command": "npm install", "cwd": "/tmp/project" })
    }

    #[test]
    fn first_response_wins() {
        let db = Database::open_in_memory().unwrap();
        let (_dir, session_id) = seed_session(&db);
        let repo = ApprovalRepository::new(&db);
        let phone = PairedDeviceRepository::new(&db).create("phone").unwrap();

        let request = repo
            .create(
                session_id,
                ApprovalKind::Command,
                &details(),
                StdDuration::from_secs(300),
            )
            .unwrap();
        assert_eq!(repo.pending(session_id).unwrap().len(), 1);

        let resolved = repo
            .resolve(request.id, Decision::Approved, Some(phone.id))
            .unwrap();
        assert_eq!(resolved.resolution, Some(Decision::Approved));
        assert!(repo.pending(session_id).unwrap().is_empty());

        match repo.resolve(request.id, Decision::Denied, None) {
            Err(Error::AlreadyResolved { resolved_by }) => assert_eq!(resolved_by, "phone"),
            other => panic!("expected AlreadyResolved, got {other:?}"),
        }
        // The original decision is untouched.
        assert_eq!(
            repo.get(request.id).unwrap().resolution,
            Some(Decision::Approved)
        );
    }

    #[test]
    fn overdue_requests_are_denied() {
        let db = Database::open_in_memory().unwrap();
        let (_dir, session_id) = seed_session(&db);
        let repo = ApprovalRepository::new(&db);

        let request = repo
            .create(
                session_id,
                ApprovalKind::FileChange,
                &serde_json::json!({ "path": "/tmp/project/a.txt", "operation": "update" }),
                StdDuration::from_millis(1),
            )
            .unwrap();
        assert!(request.is_expired(Utc::now() + Duration::seconds(1)));

        std::thread::sleep(StdDuration::from_millis(5));
        let denied = repo.expire_overdue().unwrap();
        assert_eq!(denied.len(), 1);
        assert_eq!(denied[0].resolution, Some(Decision::Denied));
        assert!(repo.expire_overdue().unwrap().is_empty());
    }

    #[test]
    fn rejects_non_positive_timeout() {
        let db = Database::open_in_memory().unwrap();
        let (_dir, session_id) = seed_session(&db);
        let repo = ApprovalRepository::new(&db);
        assert!(matches!(
            repo.create(
                session_id,
                ApprovalKind::Command,
                &details(),
                StdDuration::ZERO
            ),
            Err(Error::Validation(_))
        ));
    }
}
