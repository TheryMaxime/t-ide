//! Active WebSocket connection tracking per session (T015, FR-021).

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::storage::{now, Database};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionClientConnection {
    pub id: i64,
    pub session_id: i64,
    /// `None` for the desktop IDE itself.
    pub device_id: Option<i64>,
    pub connected_at: String,
    pub disconnected_at: Option<String>,
}

impl SessionClientConnection {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            session_id: row.get("session_id")?,
            device_id: row.get("device_id")?,
            connected_at: row.get("connected_at")?,
            disconnected_at: row.get("disconnected_at")?,
        })
    }
}

const SELECT: &str = "SELECT id, session_id, device_id, connected_at, disconnected_at FROM \
                      session_client_connections";

pub struct ConnectionRepository<'a> {
    db: &'a Database,
}

impl<'a> ConnectionRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Record a client connecting to a session. A device may only have one
    /// active connection per session, so any previous one is closed first.
    pub fn open(&self, session_id: i64, device_id: Option<i64>) -> Result<SessionClientConnection> {
        let conn = self.db.conn();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE session_client_connections SET disconnected_at = ?3 WHERE session_id = ?1 \
             AND device_id IS ?2 AND disconnected_at IS NULL",
            params![session_id, device_id, now()],
        )?;
        tx.execute(
            "INSERT INTO session_client_connections (session_id, device_id, connected_at) \
             VALUES (?1, ?2, ?3)",
            params![session_id, device_id, now()],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        drop(conn);
        self.get(id)
    }

    pub fn get(&self, id: i64) -> Result<SessionClientConnection> {
        self.db
            .conn()
            .query_row(
                &format!("{SELECT} WHERE id = ?1"),
                [id],
                SessionClientConnection::from_row,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("connection {id}")),
                other => other.into(),
            })
    }

    /// Mark a single connection as disconnected.
    pub fn close(&self, id: i64) -> Result<()> {
        self.db.conn().execute(
            "UPDATE session_client_connections SET disconnected_at = ?2 WHERE id = ?1 AND \
             disconnected_at IS NULL",
            params![id, now()],
        )?;
        Ok(())
    }

    /// Close every active connection belonging to a device, used when a device
    /// is revoked (FR-011).
    pub fn close_for_device(&self, device_id: i64) -> Result<usize> {
        let closed = self.db.conn().execute(
            "UPDATE session_client_connections SET disconnected_at = ?2 WHERE device_id = ?1 AND \
             disconnected_at IS NULL",
            params![device_id, now()],
        )?;
        Ok(closed)
    }

    /// Connections currently attached to a session.
    pub fn active(&self, session_id: i64) -> Result<Vec<SessionClientConnection>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE session_id = ?1 AND disconnected_at IS NULL ORDER BY connected_at"
        ))?;
        let rows = stmt
            .query_map([session_id], SessionClientConnection::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::pairing::PairedDeviceRepository;
    use crate::test_support::seed_session;

    #[test]
    fn tracks_one_active_connection_per_device() {
        let db = Database::open_in_memory().unwrap();
        let (_dir, session_id) = seed_session(&db);
        let device = PairedDeviceRepository::new(&db).create("phone").unwrap();
        let repo = ConnectionRepository::new(&db);

        repo.open(session_id, Some(device.id)).unwrap();
        let desktop = repo.open(session_id, None).unwrap();
        assert_eq!(repo.active(session_id).unwrap().len(), 2);

        // Reconnecting the phone replaces its previous connection.
        repo.open(session_id, Some(device.id)).unwrap();
        assert_eq!(repo.active(session_id).unwrap().len(), 2);

        repo.close(desktop.id).unwrap();
        assert_eq!(repo.active(session_id).unwrap().len(), 1);

        assert_eq!(repo.close_for_device(device.id).unwrap(), 1);
        assert!(repo.active(session_id).unwrap().is_empty());
    }
}
