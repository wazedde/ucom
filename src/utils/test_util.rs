//! Helpers shared by unit tests.

use std::fs;
use std::path::{Path, PathBuf};

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
