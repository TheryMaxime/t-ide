//! Structured error type shared by every backend layer.
//!
//! Error codes map onto the WebSocket protocol error codes defined in
//! `contracts/websocket-protocol.md`.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0} not found")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Validation(String),

    #[error("path {path} is outside the project folder")]
    SandboxViolation { path: PathBuf },

    #[error("a session is already running for project {project}")]
    SessionBusy { project: String },

    #[error("session is not running")]
    SessionNotActive,

    #[error("this request was already answered by {resolved_by}")]
    AlreadyResolved { resolved_by: String },

    #[error("auth token is invalid or revoked")]
    InvalidToken,

    #[error("model provider error: {0}")]
    Provider(String),

    #[error("protocol error: {0}")]
    Protocol(String),
}

impl Error {
    /// Protocol error code for messages sent back to clients.
    pub fn code(&self) -> &'static str {
        match self {
            Error::SessionBusy { .. } => "SESSION_BUSY",
            Error::SessionNotActive => "SESSION_NOT_ACTIVE",
            Error::AlreadyResolved { .. } => "ALREADY_RESOLVED",
            Error::InvalidToken => "INVALID_TOKEN",
            Error::Provider(_) => "PROVIDER_ERROR",
            Error::SandboxViolation { .. } => "SANDBOX_VIOLATION",
            Error::NotFound(_) => "NOT_FOUND",
            Error::Validation(_) => "INVALID_REQUEST",
            _ => "INTERNAL_ERROR",
        }
    }
}
