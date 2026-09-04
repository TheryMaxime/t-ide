//! Session transcript storage (T013).

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::storage::{now, Database};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryType {
    Prompt,
    Response,
    FileChange,
    Command,
    ApprovalDecision,
    Error,
}

impl EntryType {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryType::Prompt => "prompt",
            EntryType::Response => "response",
            EntryType::FileChange => "file_change",
            EntryType::Command => "command",
            EntryType::ApprovalDecision => "approval_decision",
            EntryType::Error => "error",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "prompt" => Ok(EntryType::Prompt),
            "response" => Ok(EntryType::Response),
            "file_change" => Ok(EntryType::FileChange),
            "command" => Ok(EntryType::Command),
            "approval_decision" => Ok(EntryType::ApprovalDecision),
            "error" => Ok(EntryType::Error),
            other => Err(Error::Validation(format!("unknown entry type {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptEntry {
    pub id: i64,
    pub session_id: i64,
    pub order_index: i64,
    pub entry_type: EntryType,
    pub origin_device_id: Option<i64>,
    /// Type-specific JSON payload (see data-model.md).
    pub content: serde_json::Value,
    pub created_at: String,
}

impl TranscriptEntry {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let entry_type: String = row.get("entry_type")?;
        let content: String = row.get("content")?;
        Ok(Self {
            id: row.get("id")?,
            session_id: row.get("session_id")?,
            order_index: row.get("order_index")?,
            entry_type: EntryType::parse(&entry_type).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    err.into(),
                )
            })?,
            origin_device_id: row.get("origin_device_id")?,
            content: serde_json::from_str(&content).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?,
            created_at: row.get("created_at")?,
        })
    }
}

const SELECT: &str = "SELECT id, session_id, order_index, entry_type, origin_device_id, content, \
                      created_at FROM transcript_entries";

pub struct TranscriptRepository<'a> {
    db: &'a Database,
}

impl<'a> TranscriptRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Append an entry, assigning the next `order_index` for the session.
    pub fn append(
        &self,
        session_id: i64,
        entry_type: EntryType,
        origin_device_id: Option<i64>,
        content: &serde_json::Value,
    ) -> Result<TranscriptEntry> {
        let serialized = serde_json::to_string(content)?;
        let conn = self.db.conn();
        let tx = conn.unchecked_transaction()?;
        let next_index: i64 = tx.query_row(
            "SELECT COALESCE(MAX(order_index), -1) + 1 FROM transcript_entries WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO transcript_entries (session_id, order_index, entry_type, \
             origin_device_id, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                next_index,
                entry_type.as_str(),
                origin_device_id,
                serialized,
                now()
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        drop(conn);
        self.get(id)
    }

    pub fn get(&self, id: i64) -> Result<TranscriptEntry> {
        self.db
            .conn()
            .query_row(
                &format!("{SELECT} WHERE id = ?1"),
                [id],
                TranscriptEntry::from_row,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    Error::NotFound(format!("transcript entry {id}"))
                }
                other => other.into(),
            })
    }

    /// All entries for a session in order.
    pub fn list(&self, session_id: i64) -> Result<Vec<TranscriptEntry>> {
        self.list_from(session_id, -1)
    }

    /// Entries with `order_index` greater than `from_index`, used by the
    /// reconnection catch-up protocol (FR-017).
    pub fn list_from(&self, session_id: i64, from_index: i64) -> Result<Vec<TranscriptEntry>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!(
            "{SELECT} WHERE session_id = ?1 AND order_index > ?2 ORDER BY order_index"
        ))?;
        let entries = stmt
            .query_map(params![session_id, from_index], TranscriptEntry::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::seed_session;

    #[test]
    fn appends_entries_in_order_and_supports_catchup() {
        let db = Database::open_in_memory().unwrap();
        let (_project_dir, session_id) = seed_session(&db);
        let repo = TranscriptRepository::new(&db);

        let first = repo
            .append(
                session_id,
                EntryType::Prompt,
                None,
                &serde_json::json!({ "text": "hi", "model": "llama3" }),
            )
            .unwrap();
        let second = repo
            .append(
                session_id,
                EntryType::Response,
                None,
                &serde_json::json!({ "text": "hello" }),
            )
            .unwrap();

        assert_eq!(first.order_index, 0);
        assert_eq!(second.order_index, 1);
        assert_eq!(repo.list(session_id).unwrap().len(), 2);

        let catchup = repo.list_from(session_id, 0).unwrap();
        assert_eq!(catchup.len(), 1);
        assert_eq!(catchup[0].id, second.id);
        assert_eq!(catchup[0].content["text"], "hello");
    }
}
