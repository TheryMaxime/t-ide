//! Model provider registry and provider abstraction scaffold (T010).
//!
//! Credentials are never stored here: they live in the OS keychain keyed by
//! provider id (see `security::credentials`).

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::storage::{now, Database};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Hosted, OpenAI-compatible API reached over the internet.
    External,
    /// Model served locally on the developer machine.
    Local,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderKind::External => "external",
            ProviderKind::Local => "local",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "external" => Ok(ProviderKind::External),
            "local" => Ok(ProviderKind::Local),
            other => Err(Error::Validation(format!("unknown provider kind {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProvider {
    pub id: i64,
    pub name: String,
    pub kind: ProviderKind,
    pub api_base_url: String,
    pub default_model: String,
    pub created_at: String,
    pub updated_at: String,
}

impl ModelProvider {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        let kind: String = row.get("kind")?;
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            kind: ProviderKind::parse(&kind).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    err.into(),
                )
            })?,
            api_base_url: row.get("api_base_url")?,
            default_model: row.get("default_model")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

const SELECT: &str = "SELECT id, name, kind, api_base_url, default_model, created_at, updated_at \
                      FROM model_providers";

pub struct ModelProviderRepository<'a> {
    db: &'a Database,
}

impl<'a> ModelProviderRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn create(
        &self,
        name: &str,
        kind: &str,
        api_base_url: &str,
        default_model: &str,
    ) -> Result<ModelProvider> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Validation("provider name must not be empty".into()));
        }
        let kind = ProviderKind::parse(kind)?;
        if !(api_base_url.starts_with("http://") || api_base_url.starts_with("https://")) {
            return Err(Error::Validation(format!(
                "provider api_base_url {api_base_url} must be an http(s) URL"
            )));
        }
        if default_model.trim().is_empty() {
            return Err(Error::Validation("default model must not be empty".into()));
        }

        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO model_providers (name, kind, api_base_url, default_model, created_at, \
             updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![name, kind.as_str(), api_base_url, default_model, now()],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get(id)
    }

    pub fn get(&self, id: i64) -> Result<ModelProvider> {
        self.db
            .conn()
            .query_row(
                &format!("{SELECT} WHERE id = ?1"),
                [id],
                ModelProvider::from_row,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("provider {id}")),
                other => other.into(),
            })
    }

    pub fn list(&self) -> Result<Vec<ModelProvider>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY name"))?;
        let providers = stmt
            .query_map([], ModelProvider::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(providers)
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        let changed = self
            .db
            .conn()
            .execute("DELETE FROM model_providers WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(Error::NotFound(format!("provider {id}")));
        }
        Ok(())
    }
}

/// A single message exchanged with a model provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderMessage {
    pub role: String,
    pub content: String,
}

/// Request sent to a model provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<ProviderMessage>,
}

/// Response returned by a model provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub text: String,
}

/// Abstraction over external and local providers. The HTTP implementation is
/// added with User Story 1 (T027).
#[allow(async_fn_in_trait)]
pub trait ProviderClient {
    /// Run a completion request against the provider.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_lists_providers() {
        let db = Database::open_in_memory().unwrap();
        let repo = ModelProviderRepository::new(&db);

        let provider = repo
            .create("OpenAI", "external", "https://api.openai.com/v1", "gpt-4")
            .unwrap();
        assert_eq!(provider.kind, ProviderKind::External);
        assert_eq!(repo.list().unwrap().len(), 1);

        repo.remove(provider.id).unwrap();
        assert!(matches!(repo.get(provider.id), Err(Error::NotFound(_))));
    }

    #[test]
    fn validates_input() {
        let db = Database::open_in_memory().unwrap();
        let repo = ModelProviderRepository::new(&db);

        assert!(matches!(
            repo.create("", "external", "https://x", "gpt-4"),
            Err(Error::Validation(_))
        ));
        assert!(matches!(
            repo.create("p", "quantum", "https://x", "gpt-4"),
            Err(Error::Validation(_))
        ));
        assert!(matches!(
            repo.create("p", "local", "ftp://x", "gpt-4"),
            Err(Error::Validation(_))
        ));
    }
}
