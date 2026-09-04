//! Project file system sandbox (T016, FR-004).
//!
//! Every agent file access goes through [`Sandbox`], which resolves paths
//! (including symlinks and `..` segments) and refuses anything that escapes the
//! registered project folder. Refusals are logged.

use std::path::{Component, Path, PathBuf};

use crate::{Error, Result};

/// Bounds all agent file access to a single project folder.
#[derive(Debug, Clone)]
pub struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    /// Create a sandbox rooted at an existing project directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(Error::Validation(format!(
                "project path {} is not a directory",
                root.display()
            )));
        }
        Ok(Self {
            root: root.canonicalize()?,
        })
    }

    /// The canonical project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve `path` (absolute, or relative to the project root) to a
    /// canonical path inside the sandbox.
    ///
    /// Returns [`Error::SandboxViolation`] for anything that resolves outside
    /// the project folder, including via symlinks or `..` segments.
    pub fn resolve(&self, path: impl AsRef<Path>) -> Result<PathBuf> {
        let requested = path.as_ref();
        let candidate = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.root.join(requested)
        };

        let resolved = canonicalize_lexically(&candidate);
        if !resolved.starts_with(&self.root) {
            tracing::warn!(
                requested = %requested.display(),
                resolved = %resolved.display(),
                root = %self.root.display(),
                "refused file access outside the project folder"
            );
            return Err(Error::SandboxViolation {
                path: requested.to_path_buf(),
            });
        }
        Ok(resolved)
    }

    /// Whether `path` is inside the sandbox.
    pub fn contains(&self, path: impl AsRef<Path>) -> bool {
        self.resolve(path).is_ok()
    }

    /// Read a file from inside the project folder.
    pub fn read_to_string(&self, path: impl AsRef<Path>) -> Result<String> {
        let path = self.resolve(path)?;
        Ok(std::fs::read_to_string(path)?)
    }

    /// Write a file inside the project folder, creating parent directories.
    pub fn write(&self, path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> Result<PathBuf> {
        let path = self.resolve(path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, contents)?;
        Ok(path)
    }

    /// Delete a file inside the project folder.
    pub fn remove_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = self.resolve(path)?;
        std::fs::remove_file(path)?;
        Ok(())
    }
}

/// Canonicalize the deepest existing ancestor of `path` (which resolves
/// symlinks) and re-append the remaining components lexically, so paths that do
/// not exist yet can still be checked.
fn canonicalize_lexically(path: &Path) -> PathBuf {
    let normalized = normalize(path);
    let mut existing = normalized.as_path();
    let mut suffix: Vec<&std::ffi::OsStr> = Vec::new();

    loop {
        if let Ok(canonical) = existing.canonicalize() {
            let mut result = canonical;
            for component in suffix.iter().rev() {
                result.push(component);
            }
            return result;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                suffix.push(name);
                existing = parent;
            }
            _ => return normalized,
        }
    }
}

/// Remove `.` and resolve `..` segments without touching the file system.
fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    result.push("..");
                }
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_paths_inside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let sandbox = Sandbox::new(dir.path()).unwrap();

        let written = sandbox.write("src/main.rs", "fn main() {}").unwrap();
        assert!(written.starts_with(sandbox.root()));
        assert_eq!(
            sandbox.read_to_string("src/main.rs").unwrap(),
            "fn main() {}"
        );
        assert!(sandbox.contains(dir.path().join("new-file.txt")));

        sandbox.remove_file("src/main.rs").unwrap();
        assert!(sandbox.read_to_string("src/main.rs").is_err());
    }

    #[test]
    fn refuses_paths_outside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        let sandbox = Sandbox::new(dir.path()).unwrap();

        for path in [
            outside.path().join("secret.txt"),
            PathBuf::from("../escape.txt"),
            PathBuf::from("nested/../../escape.txt"),
            PathBuf::from("/etc/passwd"),
        ] {
            assert!(
                matches!(sandbox.resolve(&path), Err(Error::SandboxViolation { .. })),
                "expected refusal for {}",
                path.display()
            );
        }
        assert!(sandbox.read_to_string("../escape.txt").is_err());
        assert!(sandbox.write("../escape.txt", "nope").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinks_that_escape_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.txt"),
            dir.path().join("link.txt"),
        )
        .unwrap();

        let sandbox = Sandbox::new(dir.path()).unwrap();
        assert!(matches!(
            sandbox.read_to_string("link.txt"),
            Err(Error::SandboxViolation { .. })
        ));
    }

    #[test]
    fn rejects_non_directory_roots() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(matches!(
            Sandbox::new(file.path()),
            Err(Error::Validation(_))
        ));
    }
}
