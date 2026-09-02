//! Axum HTTP/WSS server skeleton (T018) and message dispatch (T019).
//!
//! Handlers for individual message types are added by the user story tasks
//! (sessions/prompts in US1, pairing in US2, resume/streaming in US3); this
//! module owns the transport, the connection lifecycle, and the routing table.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::agent::session::SessionLock;
use crate::config::AppConfig;
use crate::network::protocol::{ClientMessage, ConnectionState, ServerMessage};
use crate::storage::Database;
use crate::{Error, Result};

/// Paths to the TLS certificate and private key used for WSS.
#[derive(Debug, Clone)]
pub struct TlsPaths {
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}

impl TlsPaths {
    /// Default TLS material location inside the application data directory.
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            certificate: config.data_dir.join("tls/cert.pem"),
            private_key: config.data_dir.join("tls/key.pem"),
        }
    }

    /// Whether both files exist on disk.
    pub fn is_present(&self) -> bool {
        self.certificate.is_file() && self.private_key.is_file()
    }
}

/// Shared state handed to every request handler.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<AppConfig>,
    pub session_lock: SessionLock,
}

impl AppState {
    pub fn new(db: Arc<Database>, config: Arc<AppConfig>) -> Self {
        Self {
            db,
            config,
            session_lock: SessionLock::new(),
        }
    }
}

/// Build the router: a health probe plus the `/ws` protocol endpoint.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn ws_upgrade(State(state): State<AppState>, upgrade: WebSocketUpgrade) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| connection(socket, state))
}

/// Serve the router over TLS, or plain HTTP when no TLS material is available.
///
/// The mobile app always connects over WSS; the plain-HTTP fallback exists so
/// the desktop IDE can talk to its own backend on the loopback interface before
/// a certificate has been provisioned.
pub async fn serve(state: AppState, tls: Option<TlsPaths>) -> Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], state.config.port));
    let app = router(state);

    match tls {
        Some(tls) if tls.is_present() => {
            let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
                &tls.certificate,
                &tls.private_key,
            )
            .await
            .map_err(|err| Error::Protocol(format!("failed to load TLS material: {err}")))?;
            tracing::info!(%addr, "listening on wss");
            axum_server::bind_rustls(addr, config)
                .serve(app.into_make_service())
                .await?;
        }
        _ => {
            tracing::warn!(%addr, "no TLS material found, listening on plain http");
            axum_server::bind(addr)
                .serve(app.into_make_service())
                .await?;
        }
    }
    Ok(())
}

/// Read frames from a client until it disconnects.
async fn connection(mut socket: WebSocket, state: AppState) {
    let _ = socket
        .send(Message::Text(
            encode(&ServerMessage::ConnectionState {
                state: ConnectionState::Connected,
            })
            .into(),
        ))
        .await;

    while let Some(frame) = socket.recv().await {
        let text = match frame {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) | Err(_) => break,
            // Ping/Pong are handled by axum; binary frames are not part of the
            // protocol.
            Ok(_) => continue,
        };

        let (reply, close) = match parse(&text) {
            Ok(ClientMessage::Disconnect) => (None, true),
            Ok(message) => match dispatch(&state, message).await {
                Ok(reply) => (reply, false),
                Err(err) => (Some(ServerMessage::from_error(&err)), false),
            },
            Err(err) => (Some(ServerMessage::from_error(&err)), false),
        };

        if let Some(reply) = reply {
            if socket
                .send(Message::Text(encode(&reply).into()))
                .await
                .is_err()
            {
                break;
            }
        }
        if close {
            break;
        }
    }

    let _ = socket.send(Message::Close(None)).await;
}

/// Parse a raw text frame into a protocol message.
pub fn parse(raw: &str) -> Result<ClientMessage> {
    serde_json::from_str(raw).map_err(|err| Error::Protocol(format!("invalid message: {err}")))
}

/// Serialize a server message for the wire.
pub fn encode(message: &ServerMessage) -> String {
    serde_json::to_string(message).expect("server messages are always serializable")
}

/// Route a parsed message to its handler.
///
/// Returns the reply to send back, if any.
pub async fn dispatch(_state: &AppState, message: ClientMessage) -> Result<Option<ServerMessage>> {
    match message {
        // Client-originated status updates need no reply.
        ClientMessage::ConnectionState { state } => {
            tracing::debug!(?state, "client reported connection state");
            Ok(None)
        }
        ClientMessage::Disconnect => Ok(None),
        other => Err(Error::Protocol(format!(
            "no handler registered for {}",
            message_name(&other)
        ))),
    }
}

fn message_name(message: &ClientMessage) -> &'static str {
    match message {
        ClientMessage::AuthVerify { .. } => "auth.verify",
        ClientMessage::SessionSubscribe { .. } => "session.subscribe",
        ClientMessage::SessionList => "session.list",
        ClientMessage::SessionCreate { .. } => "session.create",
        ClientMessage::SessionCancel { .. } => "session.cancel",
        ClientMessage::SessionResume { .. } => "session.resume",
        ClientMessage::PromptSend { .. } => "prompt.send",
        ClientMessage::ApprovalRespond { .. } => "approval.respond",
        ClientMessage::DevicePairRequest { .. } => "device.pair.request",
        ClientMessage::DeviceList => "device.list",
        ClientMessage::DeviceRevoke { .. } => "device.revoke",
        ClientMessage::ProjectsList => "projects.list",
        ClientMessage::ConnectionState { .. } => "connection.state",
        ClientMessage::Disconnect => "disconnect",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig::with_data_dir(dir.path()).unwrap();
        let state = AppState::new(
            Arc::new(Database::open_in_memory().unwrap()),
            Arc::new(config),
        );
        (dir, state)
    }

    #[test]
    fn rejects_malformed_frames() {
        assert!(matches!(parse("not json"), Err(Error::Protocol(_))));
        assert!(matches!(
            parse(r#"{ "type": "session.create" }"#),
            Err(Error::Protocol(_))
        ));
    }

    #[tokio::test]
    async fn dispatch_acknowledges_connection_state_without_a_reply() {
        let (_dir, state) = state();
        let reply = dispatch(
            &state,
            ClientMessage::ConnectionState {
                state: ConnectionState::Reconnecting,
            },
        )
        .await
        .unwrap();
        assert!(reply.is_none());
    }

    #[tokio::test]
    async fn dispatch_reports_unhandled_messages() {
        let (_dir, state) = state();
        let err = dispatch(&state, ClientMessage::SessionList)
            .await
            .unwrap_err();
        assert_eq!(err.code(), "INTERNAL_ERROR");
        assert!(err.to_string().contains("session.list"));
    }

    #[test]
    fn tls_paths_derive_from_the_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let config = AppConfig::with_data_dir(dir.path()).unwrap();
        let tls = TlsPaths::from_config(&config);
        assert!(!tls.is_present());
        assert_eq!(tls.certificate, dir.path().join("tls/cert.pem"));
    }
}
