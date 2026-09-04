//! Project registry (T009).

use std::path::Path;

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::storage::{now, Database};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub auto_approve_read_only: bool,
    pub available: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl Project {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            name: row.get("name")?,
            path: row.get("path")?,
            auto_approve_read_only: row.get("auto_approve_read_only")?,
            available: row.get("available")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

const SELECT: &str = "SELECT id, name, path, auto_approve_read_only, available, created_at, \
                      updated_at FROM projects";

/// CRUD for registered project folders.
pub struct ProjectRepository<'a> {
    db: &'a Database,
}

impl<'a> ProjectRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Register a project folder. The path must be an absolute, existing
    /// directory and unique across projects.
    pub fn create(&self, name: &str, path: &str) -> Result<Project> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Validation("project name must not be empty".into()));
        }

        let dir = Path::new(path);
        if !dir.is_absolute() {
            return Err(Error::Validation(format!(
                "project path {path} must be absolute"
            )));
        }
        if !dir.is_dir() {
            return Err(Error::Validation(format!(
                "project path {path} is not a directory"
            )));
        }

        let timestamp = now();
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO projects (name, path, auto_approve_read_only, available, created_at, \
             updated_at) VALUES (?1, ?2, 0, 1, ?3, ?3)",
            params![name, path, timestamp],
        )
        .map_err(map_unique_violation)?;

        let id = conn.last_insert_rowid();
        drop(conn);
        self.get(id)
    }

    pub fn get(&self, id: i64) -> Result<Project> {
        self.db
            .conn()
            .query_row(&format!("{SELECT} WHERE id = ?1"), [id], Project::from_row)
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("project {id}")),
                other => other.into(),
            })
    }

    pub fn find_by_path(&self, path: &str) -> Result<Option<Project>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} WHERE path = ?1"))?;
        let mut rows = stmt.query_map([path], Project::from_row)?;
        rows.next().transpose().map_err(Into::into)
    }

    pub fn list(&self) -> Result<Vec<Project>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY name"))?;
        let projects = stmt
            .query_map([], Project::from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(projects)
    }

    pub fn rename(&self, id: i64, name: &str) -> Result<Project> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Validation("project name must not be empty".into()));
        }
        let changed = self
            .db
            .conn()
            .execute(
                "UPDATE projects SET name = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, name, now()],
            )
            .map_err(map_unique_violation)?;
        if changed == 0 {
            return Err(Error::NotFound(format!("project {id}")));
        }
        self.get(id)
    }

    pub fn set_auto_approve_read_only(&self, id: i64, enabled: bool) -> Result<Project> {
        let changed = self.db.conn().execute(
            "UPDATE projects SET auto_approve_read_only = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, enabled, now()],
        )?;
        if changed == 0 {
            return Err(Error::NotFound(format!("project {id}")));
        }
        self.get(id)
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        let changed = self
            .db
            .conn()
            .execute("DELETE FROM projects WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(Error::NotFound(format!("project {id}")));
        }
        Ok(())
    }

    /// Re-check whether the folder still exists on disk (FR-016c) and persist
    /// the result.
    pub fn refresh_availability(&self, id: i64) -> Result<Project> {
        let project = self.get(id)?;
        let available = Path::new(&project.path).is_dir();
        if available != project.available {
            self.db.conn().execute(
                "UPDATE projects SET available = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, available, now()],
            )?;
            tracing::info!(
                project = project.name,
                available,
                "project availability changed"
            );
        }
        self.get(id)
    }
}

fn map_unique_violation(err: rusqlite::Error) -> Error {
    match &err {
        rusqlite::Error::SqliteFailure(failure, Some(message))
            if failure.code == rusqlite::ErrorCode::ConstraintViolation
                && message.contains("UNIQUE") =>
        {
            Error::Validation(format!("project already registered ({message})"))
        }
        _ => err.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn creates_lists_and_renames_projects() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let db = db();
        let repo = ProjectRepository::new(&db);

        let project = repo.create("demo", path).unwrap();
        assert!(project.available);
        assert!(!project.auto_approve_read_only);
        assert_eq!(repo.list().unwrap().len(), 1);

        let renamed = repo.rename(project.id, "renamed").unwrap();
        assert_eq!(renamed.name, "renamed");

        let updated = repo.set_auto_approve_read_only(project.id, true).unwrap();
        assert!(updated.auto_approve_read_only);

        repo.remove(project.id).unwrap();
        assert!(repo.list().unwrap().is_empty());
    }

    #[test]
    fn rejects_duplicate_paths_and_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        let db = db();
        let repo = ProjectRepository::new(&db);

        repo.create("demo", path).unwrap();
        assert!(matches!(
            repo.create("other", path),
            Err(Error::Validation(_))
        ));
        assert!(matches!(repo.create("", path), Err(Error::Validation(_))));
        assert!(matches!(
            repo.create("relative", "not/absolute"),
            Err(Error::Validation(_))
        ));
    }

    #[test]
    fn marks_missing_folder_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let db = db();
        let repo = ProjectRepository::new(&db);
        let project = repo.create("demo", &path).unwrap();

        dir.close().unwrap();
        let refreshed = repo.refresh_availability(project.id).unwrap();
        assert!(!refreshed.available);
    }
}
