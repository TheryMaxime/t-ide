//! Axum HTTP/WSS server (T018), message dispatch (T019), transcript streaming
//! (T029), and the session/prompt handlers of User Story 1 (T033).
//!
//! Pairing handlers arrive with User Story 2 and the resume protocol with User
//! Story 3; this module owns the transport, the connection lifecycle, and the
//! routing table.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

use crate::agent::approval::ApprovalService;
use crate::agent::provider::{
    HttpProviderClient, ModelProvider, ModelProviderRepository, SharedProvider,
};
use crate::agent::session::{CancellationRegistry, SessionLock, SessionRepository, SessionStatus};
use crate::agent::AgentEngine;
use crate::config::AppConfig;
use crate::network::events::SessionEvents;
use crate::network::protocol::{ClientMessage, ConnectionState, ServerMessage, SessionSummary};
use crate::security::credentials::{CredentialStore, InMemoryCredentialStore};
use crate::storage::projects::ProjectRepository;
use crate::storage::Database;
use crate::{Error, Result};

/// Builds the client used to talk to a model provider.
///
/// Tests replace the factory with an in-process fake so no network traffic is
/// needed.
pub type ProviderFactory = Arc<dyn Fn(&ModelProvider) -> Result<SharedProvider> + Send + Sync>;

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
    pub events: SessionEvents,
    pub approvals: ApprovalService,
    pub cancellations: CancellationRegistry,
    pub engine: AgentEngine,
    pub providers: ProviderFactory,
}

impl AppState {
    pub fn new(db: Arc<Database>, config: Arc<AppConfig>) -> Self {
        Self::with_credentials(db, config, Arc::new(InMemoryCredentialStore::new()))
    }

    /// Build the state with a specific credential store (the OS keychain in
    /// production, an in-memory store in tests).
    pub fn with_credentials(
        db: Arc<Database>,
        config: Arc<AppConfig>,
        credentials: Arc<dyn CredentialStore>,
    ) -> Self {
        let events = SessionEvents::new();
        let approvals = ApprovalService::new(db.clone(), events.clone());
        let cancellations = CancellationRegistry::new();
        let engine = AgentEngine::new(
            db.clone(),
            events.clone(),
            approvals.clone(),
            cancellations.clone(),
            config.approval_timeout(),
        );
        Self {
            db,
            config,
            session_lock: SessionLock::new(),
            events,
            approvals,
            cancellations,
            engine,
            providers: http_provider_factory(credentials),
        }
    }

    /// Replace the provider factory (used by tests and by local model setups).
    pub fn with_provider_factory(mut self, factory: ProviderFactory) -> Self {
        self.providers = factory;
        self
    }
}

/// Default factory: an OpenAI-compatible HTTP client authenticated with the
/// credential stored for the provider. The secret never leaves this closure.
fn http_provider_factory(credentials: Arc<dyn CredentialStore>) -> ProviderFactory {
    Arc::new(move |provider: &ModelProvider| {
        let api_key = credentials.get(provider.id)?;
        Ok(Arc::new(HttpProviderClient::new(provider, api_key)?) as SharedProvider)
    })
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

/// Read frames from a client while forwarding streamed agent output to it
/// (T029) until it disconnects.
async fn connection(mut socket: WebSocket, state: AppState) {
    let mut events = state.events.subscribe();
    let _ = socket
        .send(Message::Text(
            encode(&ServerMessage::ConnectionState {
                state: ConnectionState::Connected,
            })
            .into(),
        ))
        .await;

    loop {
        let (reply, close) = tokio::select! {
            streamed = events.recv() => match streamed {
                Ok(message) => (Some(message), false),
                // The client fell behind; it recovers with transcript.catchup.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "client lagged behind the event stream");
                    (None, false)
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => (None, true),
            },
            frame = socket.recv() => match frame {
                Some(Ok(Message::Text(text))) => match parse(&text) {
                    Ok(ClientMessage::Disconnect) => (None, true),
                    Ok(message) => match dispatch(&state, message).await {
                        Ok(reply) => (reply, false),
                        Err(err) => (Some(ServerMessage::from_error(&err)), false),
                    },
                    Err(err) => (Some(ServerMessage::from_error(&err)), false),
                },
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => (None, true),
                // Ping/Pong are handled by axum; binary frames are not part of
                // the protocol.
                Some(Ok(_)) => (None, false),
            },
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
pub async fn dispatch(state: &AppState, message: ClientMessage) -> Result<Option<ServerMessage>> {
    match message {
        ClientMessage::SessionList => Ok(Some(ServerMessage::SessionListResponse {
            sessions: session_summaries(state)?,
        })),

        ClientMessage::SessionCreate {
            project_id,
            provider_id,
            prompt,
        } => {
            let session =
                SessionRepository::new(&state.db).create(project_id, provider_id, &prompt, None)?;
            start_prompt(state, session.id, prompt, None, None)?;
            Ok(Some(ServerMessage::SessionCreated {
                session_id: session.id,
                status: SessionStatus::Running.as_str().to_string(),
            }))
        }

        ClientMessage::PromptSend {
            session_id,
            text,
            model,
        } => {
            let session = SessionRepository::new(&state.db).get(session_id)?;
            if session.status.is_terminal() {
                return Err(Error::SessionNotActive);
            }
            start_prompt(state, session_id, text, None, model)?;
            Ok(Some(ServerMessage::PromptSent { session_id }))
        }

        ClientMessage::SessionCancel { session_id } => {
            let sessions = SessionRepository::new(&state.db);
            let session = sessions.get(session_id)?;
            if session.status.is_terminal() {
                return Err(Error::SessionNotActive);
            }
            // The running prompt marks the session interrupted itself; a
            // session with no prompt in flight is stopped here.
            if !state.cancellations.cancel(session_id) {
                sessions.update_status(session_id, SessionStatus::Interrupted)?;
            }
            Ok(Some(ServerMessage::SessionCancelled { session_id }))
        }

        ClientMessage::ApprovalRespond {
            request_id,
            decision,
        } => {
            let request = state.approvals.respond(request_id, decision, None)?;
            Ok(Some(ServerMessage::ApprovalResponded {
                request_id: request.id,
                resolved_by_device_name: "desktop".into(),
                decision,
            }))
        }

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

/// Start an agent run for `session_id` in the background.
///
/// The reply to the client is sent immediately; the prompt output arrives as
/// streamed `transcript.entry` frames.
fn start_prompt(
    state: &AppState,
    session_id: i64,
    prompt: String,
    origin_device_id: Option<i64>,
    model: Option<String>,
) -> Result<()> {
    let sessions = SessionRepository::new(&state.db);
    let session = sessions.get(session_id)?;
    // Only one session may run at a time across all projects (FR-022).
    if let Some(active) = sessions.active()? {
        if active.id != session_id {
            let project = ProjectRepository::new(&state.db).get(active.project_id)?;
            return Err(Error::SessionBusy {
                project: project.name,
            });
        }
    }

    let provider_row = ModelProviderRepository::new(&state.db).get(session.provider_id)?;
    let provider = (state.providers)(&provider_row)?;
    let engine = state.engine.clone();
    let events = state.events.clone();
    tokio::spawn(async move {
        if let Err(err) = engine
            .run_prompt(session_id, &prompt, origin_device_id, provider, model)
            .await
        {
            tracing::error!(session = session_id, error = %err, "prompt run failed");
            events.publish(ServerMessage::from_error(&err));
            events.publish(ServerMessage::TranscriptComplete { session_id });
        }
    });
    Ok(())
}

/// Sessions with their project name, newest first.
fn session_summaries(state: &AppState) -> Result<Vec<SessionSummary>> {
    let projects = ProjectRepository::new(&state.db);
    SessionRepository::new(&state.db)
        .list()?
        .into_iter()
        .map(|session| {
            Ok(SessionSummary {
                id: session.id,
                project_name: projects.get(session.project_id)?.name,
                status: session.status.as_str().to_string(),
                prompt_text: session.prompt_text,
                created_at: session.created_at,
            })
        })
        .collect()
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
        // Pairing handlers arrive with User Story 2.
        let err = dispatch(&state, ClientMessage::DeviceList)
            .await
            .unwrap_err();
        assert_eq!(err.code(), "INTERNAL_ERROR");
        assert!(err.to_string().contains("device.list"));
    }

    #[tokio::test]
    async fn dispatch_lists_sessions() {
        let (_dir, state) = state();
        let reply = dispatch(&state, ClientMessage::SessionList).await.unwrap();
        assert!(matches!(
            reply,
            Some(ServerMessage::SessionListResponse { ref sessions }) if sessions.is_empty()
        ));
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
