//! Shared fixtures for the User Story 1 contract and integration tests.
//!
//! Each test file includes this module with `#[path = ...] mod harness;`.

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use t_ide::agent::provider::{
    CompletionRequest, CompletionResponse, ModelProviderRepository, ProviderClient, SharedProvider,
};
use t_ide::config::AppConfig;
use t_ide::network::protocol::ServerMessage;
use t_ide::network::server::AppState;
use t_ide::storage::projects::ProjectRepository;
use t_ide::storage::Database;
use tempfile::TempDir;
use tokio::sync::broadcast::Receiver;

/// How long a test waits for a streamed frame before failing.
pub const WAIT: Duration = Duration::from_secs(10);

/// Model provider stub: replies with a canned action plan, so no test ever
/// reaches the network.
pub struct ScriptedProvider {
    replies: Mutex<VecDeque<String>>,
    delay: Duration,
}

impl ScriptedProvider {
    pub fn new(replies: impl IntoIterator<Item = String>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().collect()),
            delay: Duration::ZERO,
        }
    }

    /// Make the provider hang for `delay` before replying, so cancellation can
    /// be observed while a request is in flight.
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

impl ProviderClient for ScriptedProvider {
    async fn complete(&self, _request: CompletionRequest) -> t_ide::Result<CompletionResponse> {
        tokio::time::sleep(self.delay).await;
        let reply = self
            .replies
            .lock()
            .expect("scripted provider poisoned")
            .pop_front()
            .unwrap_or_else(|| r#"{ "actions": [] }"#.to_string());
        Ok(CompletionResponse { text: reply })
    }
}

/// A backend wired to a temporary data directory, a temporary project folder,
/// and a scripted model provider.
pub struct Harness {
    pub state: AppState,
    pub project_id: i64,
    pub provider_id: i64,
    pub project_dir: TempDir,
    _data_dir: TempDir,
}

impl Harness {
    pub fn new(provider: ScriptedProvider) -> Self {
        Self::build(provider, |config| config)
    }

    /// Build a harness whose configuration is adjusted before use (for example
    /// to shorten the approval window).
    pub fn build(provider: ScriptedProvider, adjust: impl FnOnce(AppConfig) -> AppConfig) -> Self {
        let data_dir = tempfile::tempdir().expect("data dir");
        let project_dir = tempfile::tempdir().expect("project dir");
        let config = adjust(AppConfig::with_data_dir(data_dir.path()).expect("config"));

        let db = Arc::new(Database::open_in_memory().expect("database"));
        let project = ProjectRepository::new(&db)
            .create(
                "demo",
                project_dir
                    .path()
                    .canonicalize()
                    .expect("canonical project path")
                    .to_str()
                    .expect("utf-8 project path"),
            )
            .expect("project");
        let model_provider = ModelProviderRepository::new(&db)
            .create("local", "local", "http://127.0.0.1:11434", "llama3")
            .expect("provider");

        let shared: SharedProvider = Arc::new(provider);
        let state = AppState::new(db, Arc::new(config))
            .with_provider_factory(Arc::new(move |_| Ok(shared.clone())));

        Self {
            state,
            project_id: project.id,
            provider_id: model_provider.id,
            project_dir,
            _data_dir: data_dir,
        }
    }

    /// Subscribe to streamed frames before triggering the action under test.
    pub fn subscribe(&self) -> Receiver<ServerMessage> {
        self.state.events.subscribe()
    }

    /// Enable per-project auto-approval of read-only actions (FR-005).
    pub fn auto_approve_read_only(&self) {
        ProjectRepository::new(&self.state.db)
            .set_auto_approve_read_only(self.project_id, true)
            .expect("auto approve");
    }

    pub fn project_file(&self, name: &str) -> std::path::PathBuf {
        self.project_dir.path().join(name)
    }
}

/// Wait for the first streamed frame matching `predicate`.
pub async fn wait_for<T>(
    events: &mut Receiver<ServerMessage>,
    predicate: impl Fn(&ServerMessage) -> Option<T>,
) -> T {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let message = tokio::time::timeout_at(deadline, events.recv())
            .await
            .expect("timed out waiting for a streamed frame")
            .expect("event stream closed");
        if let Some(value) = predicate(&message) {
            return value;
        }
    }
}

/// A plan reply containing the given actions.
pub fn plan(actions: serde_json::Value) -> String {
    serde_json::json!({
        "reasoning": "here is what I will do",
        "actions": actions,
        "summary": "done",
    })
    .to_string()
}
