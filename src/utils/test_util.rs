//! Helpers shared by unit tests.

use std::fs;
use std::path::{Path, PathBuf};

use crate::unity::ProjectPath;
use crate::unity::project::VERSION_SUB_PATH;

/// A uniquely named directory under the system temp directory, removed when dropped,
/// including when a test panics.
pub struct TempDir(PathBuf);

impl TempDir {
    /// Creates a new empty directory whose name starts with `ucom-<label>-`.
    pub fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("ucom-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("failed to create temporary test directory");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Creates a minimal Unity project (only `ProjectVersion.txt`) in a new [`TempDir`].
/// Keep the `TempDir`: the project is removed when it is dropped.
pub fn temp_project(label: &str) -> (TempDir, ProjectPath) {
    let dir = TempDir::new(label);
    let version_file = dir.path().join(VERSION_SUB_PATH);
    fs::create_dir_all(version_file.parent().unwrap()).unwrap();
    fs::write(version_file, "m_EditorVersion: 6000.6.8f1\n").unwrap();
    let project = ProjectPath::try_from(dir.path()).expect("not detected as a Unity project");
    (dir, project)
}
