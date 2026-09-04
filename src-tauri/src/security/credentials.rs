//! Provider credential storage (T017, FR-007c/FR-007d).
//!
//! Credentials live in the OS keychain, keyed by model provider id. They are
//! never persisted in SQLite, never sent over the WebSocket protocol, and never
//! returned to the desktop frontend or the mobile app: callers only ever pass
//! them straight to the provider HTTP client.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::{Error, Result};

/// Keychain service name under which every provider secret is stored.
pub const SERVICE_NAME: &str = "t-ide";

/// Keychain account name for a provider's API credential.
pub fn account_for(provider_id: i64) -> String {
    format!("model-provider-{provider_id}")
}

/// Storage backend for provider credentials.
pub trait CredentialStore: Send + Sync {
    /// Store (or replace) the secret for a provider.
    fn set(&self, provider_id: i64, secret: &str) -> Result<()>;
    /// Fetch the secret for a provider, if one is stored.
    fn get(&self, provider_id: i64) -> Result<Option<String>>;
    /// Remove the secret for a provider. Missing secrets are not an error.
    fn delete(&self, provider_id: i64) -> Result<()>;
}

/// In-memory store used by tests and on platforms without a keychain.
///
/// Secrets are held only for the lifetime of the process.
#[derive(Debug, Default)]
pub struct InMemoryCredentialStore {
    secrets: Mutex<HashMap<i64, String>>,
}

impl InMemoryCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CredentialStore for InMemoryCredentialStore {
    fn set(&self, provider_id: i64, secret: &str) -> Result<()> {
        if secret.is_empty() {
            return Err(Error::Validation("credential must not be empty".into()));
        }
        self.secrets
            .lock()
            .expect("credential store poisoned")
            .insert(provider_id, secret.to_string());
        Ok(())
    }

    fn get(&self, provider_id: i64) -> Result<Option<String>> {
        Ok(self
            .secrets
            .lock()
            .expect("credential store poisoned")
            .get(&provider_id)
            .cloned())
    }

    fn delete(&self, provider_id: i64) -> Result<()> {
        self.secrets
            .lock()
            .expect("credential store poisoned")
            .remove(&provider_id);
        Ok(())
    }
}

/// OS keychain backed store (Keychain, Credential Manager, Secret Service).
#[cfg(feature = "os-keychain")]
#[derive(Debug, Default)]
pub struct KeyringCredentialStore;

#[cfg(feature = "os-keychain")]
impl KeyringCredentialStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(provider_id: i64) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE_NAME, &account_for(provider_id))
            .map_err(|err| Error::Validation(format!("keychain unavailable: {err}")))
    }
}

#[cfg(feature = "os-keychain")]
impl CredentialStore for KeyringCredentialStore {
    fn set(&self, provider_id: i64, secret: &str) -> Result<()> {
        if secret.is_empty() {
            return Err(Error::Validation("credential must not be empty".into()));
        }
        Self::entry(provider_id)?
            .set_password(secret)
            .map_err(|err| Error::Validation(format!("failed to store credential: {err}")))
    }

    fn get(&self, provider_id: i64) -> Result<Option<String>> {
        match Self::entry(provider_id)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(Error::Validation(format!(
                "failed to read credential: {err}"
            ))),
        }
    }

    fn delete(&self, provider_id: i64) -> Result<()> {
        match Self::entry(provider_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(Error::Validation(format!(
                "failed to delete credential: {err}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_removes_secrets() {
        let store = InMemoryCredentialStore::new();
        assert!(store.get(1).unwrap().is_none());

        store.set(1, "sk-secret").unwrap();
        assert_eq!(store.get(1).unwrap().as_deref(), Some("sk-secret"));

        store.delete(1).unwrap();
        assert!(store.get(1).unwrap().is_none());
        // Deleting a missing secret is a no-op.
        store.delete(1).unwrap();
    }

    #[test]
    fn rejects_empty_secrets() {
        let store = InMemoryCredentialStore::new();
        assert!(matches!(store.set(1, ""), Err(Error::Validation(_))));
    }

    #[test]
    fn accounts_are_scoped_per_provider() {
        assert_ne!(account_for(1), account_for(2));
    }
}
