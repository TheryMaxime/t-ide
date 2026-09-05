//! T-ide desktop backend entry point (T021, T022).
//!
//! Boots logging and configuration, opens the migrated database, and serves the
//! local WSS endpoint. The Tauri desktop shell is compiled in with the
//! `desktop` feature.

use std::sync::Arc;

use t_ide::config::AppConfig;
use t_ide::network::server::{self, AppState, TlsPaths};
use t_ide::storage::Database;
use t_ide::Result;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = AppConfig::load()?;
    config.save()?;
    tracing::info!(data_dir = %config.data_dir.display(), port = config.port, "starting t-ide");

    let db = Arc::new(Database::open(config.database_path())?);
    let tls = TlsPaths::from_config(&config);
    let state = AppState::with_credentials(
        db,
        Arc::new(config),
        t_ide::security::credentials::default_store(),
    );

    server::serve(state, Some(tls)).await
}

/// Structured logging, configurable with `RUST_LOG` (default `info`).
fn init_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}
