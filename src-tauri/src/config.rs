//! Application/environment configuration (T022).
//!
//! Configuration lives next to the SQLite database in the platform application
//! data directory. Values can be overridden with `T_IDE_*` environment
//! variables so tests and CI can run against a temporary directory.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::Result;

/// Default window before an unanswered approval request is treated as a denial
/// (FR-019).
pub const DEFAULT_APPROVAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Default port for the local HTTP/WSS server.
pub const DEFAULT_PORT: u16 = 8787;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Directory holding the database, config file, and TLS material.
    pub data_dir: PathBuf,
    /// Port the local HTTP/WSS server binds to.
    pub port: u16,
    /// Seconds before an unanswered approval request is auto-denied.
    pub approval_timeout_secs: u64,
}

impl AppConfig {
    /// Build the configuration for `data_dir`, creating the directory if needed.
    pub fn with_data_dir(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir)?;
        Ok(Self {
            data_dir,
            port: DEFAULT_PORT,
            approval_timeout_secs: DEFAULT_APPROVAL_TIMEOUT.as_secs(),
        })
    }

    /// Load the configuration from disk, falling back to defaults.
    ///
    /// `T_IDE_DATA_DIR` overrides the data directory and `T_IDE_PORT` the port.
    pub fn load() -> Result<Self> {
        let data_dir = match std::env::var("T_IDE_DATA_DIR") {
            Ok(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => default_data_dir(),
        };

        let mut config = match std::fs::read_to_string(config_path(&data_dir)) {
            Ok(raw) => {
                let mut config: AppConfig = serde_json::from_str(&raw)?;
                config.data_dir = data_dir;
                config
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                AppConfig::with_data_dir(data_dir)?
            }
            Err(err) => return Err(err.into()),
        };

        if let Ok(port) = std::env::var("T_IDE_PORT") {
            config.port = port
                .parse()
                .map_err(|_| crate::Error::Validation(format!("invalid T_IDE_PORT: {port}")))?;
        }

        Ok(config)
    }

    /// Persist the configuration to `<data_dir>/config.json`.
    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::write(
            config_path(&self.data_dir),
            serde_json::to_string_pretty(self)?,
        )?;
        Ok(())
    }

    /// Path of the SQLite database file.
    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("t-ide.db")
    }

    pub fn approval_timeout(&self) -> Duration {
        Duration::from_secs(self.approval_timeout_secs)
    }
}

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}

fn default_data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("t-ide")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = AppConfig::with_data_dir(dir.path()).unwrap();
        config.port = 9999;
        config.save().unwrap();

        let raw = std::fs::read_to_string(config_path(dir.path())).unwrap();
        let loaded: AppConfig = serde_json::from_str(&raw).unwrap();
        assert_eq!(loaded.port, 9999);
        assert_eq!(config.database_path(), dir.path().join("t-ide.db"));
    }
}
