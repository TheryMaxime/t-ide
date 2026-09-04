//! SQLite persistence layer.

pub mod connections;
pub mod pairing;
pub mod projects;
pub mod transcripts;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::Result;

/// Embedded migrations, applied in order at startup.
const MIGRATIONS: &[(&str, &str)] = &[(
    "0001_initial",
    include_str!("../../migrations/0001_initial.sql"),
)];

/// A migrated SQLite database handle shared across the backend.
#[derive(Debug)]
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open (creating if needed) and migrate the database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::from_connection(Connection::open(path)?)
    }

    /// Open and migrate an in-memory database (tests).
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Apply every migration that has not been recorded yet.
    fn migrate(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                 name TEXT PRIMARY KEY,
                 applied_at TEXT NOT NULL
             );",
        )?;

        for (name, sql) in MIGRATIONS {
            let applied: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE name = ?1)",
                [name],
                |row| row.get(0),
            )?;
            if applied {
                continue;
            }
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO schema_migrations (name, applied_at) VALUES (?1, ?2)",
                rusqlite::params![name, now()],
            )?;
            tracing::info!(migration = name, "applied migration");
        }
        Ok(())
    }

    /// Borrow the underlying connection.
    ///
    /// Panics only if another thread panicked while holding the lock, which
    /// would mean the database is in an unknown state.
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().expect("database mutex poisoned")
    }
}

/// Current time as an ISO 8601 / RFC 3339 UTC string, the storage format used
/// by every timestamp column in `data-model.md`.
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_create_all_entities_and_are_idempotent() {
        let db = Database::open_in_memory().unwrap();
        db.migrate().unwrap();

        let conn = db.conn();
        for table in [
            "projects",
            "model_providers",
            "paired_devices",
            "agent_sessions",
            "transcript_entries",
            "approval_requests",
            "session_client_connections",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "missing table {table}");
        }
    }
}
