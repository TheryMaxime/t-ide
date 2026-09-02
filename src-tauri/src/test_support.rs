//! Shared fixtures for unit tests.

use tempfile::TempDir;

use crate::agent::provider::ModelProviderRepository;
use crate::agent::session::SessionRepository;
use crate::storage::projects::ProjectRepository;
use crate::storage::Database;

/// Create a project (backed by a live temporary directory), a provider, and a
/// session, returning the directory guard and the session id.
pub fn seed_session(db: &Database) -> (TempDir, i64) {
    let (dir, project_id, provider_id) = seed_project_and_provider(db);
    let session = SessionRepository::new(db)
        .create(project_id, provider_id, "do the thing", None)
        .unwrap();
    (dir, session.id)
}

/// Create a project and a model provider, returning the project directory
/// guard plus both ids.
pub fn seed_project_and_provider(db: &Database) -> (TempDir, i64, i64) {
    let dir = tempfile::tempdir().unwrap();
    let project = ProjectRepository::new(db)
        .create("demo", dir.path().to_str().unwrap())
        .unwrap();
    let provider = ModelProviderRepository::new(db)
        .create("local", "local", "http://127.0.0.1:11434", "llama3")
        .unwrap();
    (dir, project.id, provider.id)
}
