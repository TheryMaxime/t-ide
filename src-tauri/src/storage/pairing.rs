//! Paired device registry (T011).

use rand::rand_core::TryRng;
use rand::rngs::SysRng;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::storage::{now, Database};
use crate::{Error, Result};

/// Auth tokens carry 256 bits of entropy, stored as hex (data-model.md).
const AUTH_TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: i64,
    pub device_name: String,
    /// Never sent to clients other than the device it was issued to.
    #[serde(skip_serializing)]
    pub auth_token: String,
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub revoked: bool,
    pub revoked_at: Option<String>,
}

impl PairedDevice {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            device_name: row.get("device_name")?,
            auth_token: row.get("auth_token")?,
            paired_at: row.get("paired_at")?,
            last_seen_at: row.get("last_seen_at")?,
            revoked: row.get("revoked")?,
            revoked_at: row.get("revoked_at")?,
        })
    }
}

const SELECT: &str = "SELECT id, device_name, auth_token, paired_at, last_seen_at, revoked, \
                      revoked_at FROM paired_devices";

/// Generate a cryptographically random auth token (hex encoded).
pub fn generate_auth_token() -> Result<String> {
    let mut bytes = [0u8; AUTH_TOKEN_BYTES];
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|err| Error::Validation(format!("failed to generate auth token: {err}")))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub struct PairedDeviceRepository<'a> {
    db: &'a Database,
}

impl<'a> PairedDeviceRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Pair a device, returning the record including its freshly issued token.
    pub fn create(&self, device_name: &str) -> Result<PairedDevice> {
        let device_name = device_name.trim();
        if device_name.is_empty() {
            return Err(Error::Validation("device name must not be empty".into()));
        }
        let token = generate_auth_token()?;
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO paired_devices (device_name, auth_token, paired_at, revoked) \
             VALUES (?1, ?2, ?3, 0)",
            params![device_name, token, now()],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get(id)
    }

    pub fn get(&self, id: i64) -> Result<PairedDevice> {
        self.db
            .conn()
            .query_row(
                &format!("{SELECT} WHERE id = ?1"),
                [id],
                PairedDevice::from_row,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("device {id}")),
                other => other.into(),
            })
    }

    /// Look up a device by auth token. Revoked or unknown tokens are rejected.
    pub fn authenticate(&self, token: &str) -> Result<PairedDevice> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE auth_token = ?1"))?;
        let device = stmt
            .query_map([token], PairedDevice::from_row)?
            .next()
            .transpose()?
            .ok_or(Error::InvalidToken)?;
        if device.revoked {
            return Err(Error::InvalidToken);
        }
        Ok(device)
    }

    pub fn list(&self) -> Result<Vec<PairedDevice>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY paired_at DESC"))?;
        let devices = stmt
            .query_map([], PairedDevice::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(devices)
    }

    pub fn touch_last_seen(&self, id: i64) -> Result<()> {
        self.db.conn().execute(
            "UPDATE paired_devices SET last_seen_at = ?2 WHERE id = ?1",
            params![id, now()],
        )?;
        Ok(())
    }

    /// Revoke a device. Revocation is permanent and idempotent.
    pub fn revoke(&self, id: i64) -> Result<PairedDevice> {
        let changed = self.db.conn().execute(
            "UPDATE paired_devices SET revoked = 1, revoked_at = ?2 WHERE id = ?1 AND revoked = 0",
            params![id, now()],
        )?;
        let device = self.get(id)?;
        if changed > 0 {
            tracing::info!(device = device.device_name, "device revoked");
        }
        Ok(device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_authenticates_and_revokes() {
        let db = Database::open_in_memory().unwrap();
        let repo = PairedDeviceRepository::new(&db);

        let device = repo.create("phone").unwrap();
        assert_eq!(device.auth_token.len(), AUTH_TOKEN_BYTES * 2);
        assert!(!device.revoked);

        let authenticated = repo.authenticate(&device.auth_token).unwrap();
        assert_eq!(authenticated.id, device.id);

        repo.touch_last_seen(device.id).unwrap();
        assert!(repo.get(device.id).unwrap().last_seen_at.is_some());

        let revoked = repo.revoke(device.id).unwrap();
        assert!(revoked.revoked);
        assert!(revoked.revoked_at.is_some());
        assert!(matches!(
            repo.authenticate(&device.auth_token),
            Err(Error::InvalidToken)
        ));
    }

    #[test]
    fn rejects_unknown_tokens_and_empty_names() {
        let db = Database::open_in_memory().unwrap();
        let repo = PairedDeviceRepository::new(&db);
        assert!(matches!(
            repo.authenticate("nope"),
            Err(Error::InvalidToken)
        ));
        assert!(matches!(repo.create("  "), Err(Error::Validation(_))));
    }

    #[test]
    fn tokens_are_unique() {
        assert_ne!(
            generate_auth_token().unwrap(),
            generate_auth_token().unwrap()
        );
    }
}
