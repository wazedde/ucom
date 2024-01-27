use std::borrow::Cow;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use path_absolutize::Absolutize;

pub use crate::unity::project::*;
pub use crate::unity::releases::*;
pub use crate::unity::spawn_cmd::*;
pub use crate::unity::version::*;

pub mod http_cache;
pub mod project;
pub mod releases;
pub mod spawn_cmd;
pub mod version;

const VERSION_SUB_PATH: &str = "ProjectSettings/ProjectVersion.txt";

/// Represents a valid path to a Unity project.
pub struct ProjectPath(PathBuf);

impl ProjectPath {
    /// Creates a new `ProjectPath` from the given directory.
    pub fn try_from<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = to_absolute_dir_path(&path)?;
        if ProjectPath::is_unity_project_directory(&path) {
            Ok(Self(path.as_ref().to_path_buf()))
        } else {
            Err(anyhow!(
                "Path does not contain a Unity project: {}",
                path.display()
            ))
        }
    }

    /// Returns the absolute path to the project directory.
    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }

    /// Returns the Unity version for the project in the given directory.
    pub fn unity_version(&self) -> anyhow::Result<Version> {
        let version_file = self.as_path().join(VERSION_SUB_PATH);
        let mut reader = BufReader::new(File::open(&version_file)?);

        // ProjectVersion.txt looks like this:
        // m_EditorVersion: 2021.3.9f1
        // m_EditorVersionWithRevision: 2021.3.9f1 (ad3870b89536)

        let mut line = String::new();
        // Read the 1st line.
        _ = reader.read_line(&mut line)?;

        line.starts_with("m_EditorVersion:")
            .then_some(line)
            .and_then(|l| {
                l.split(':') // Split the line,
                    .nth(1) // and return 2nd element.
                    .map(str::trim)
                    .and_then(|v| v.parse().ok())
            })
            .ok_or_else(|| {
                anyhow!(
                    "Could not get project version from `{}`",
                    version_file.display()
                )
            })
    }

    /// Checks if the project directory has an `Assets` directory.
    pub fn check_assets_directory_exists(&self) -> anyhow::Result<()> {
        let assets_path = self.as_path().join("Assets");

        if !assets_path.exists() {
            Err(anyhow!(
                "Unity project does not have an `Assets` directory: `{}`",
                self.as_path().display()
            ))
        } else {
            Ok(())
        }
    }

    /// Checks if the directory contains a Unity project.
    fn is_unity_project_directory<P: AsRef<Path>>(dir: P) -> bool {
        dir.as_ref().join(VERSION_SUB_PATH).exists()
    }
}

/// Returns the absolute path to an existing directory.
pub fn to_absolute_dir_path<P: AsRef<Path>>(path: &P) -> anyhow::Result<Cow<'_, Path>> {
    let path = path.as_ref();
    if cfg!(target_os = "windows") && path.starts_with("~") {
        return Err(anyhow!(
            "On Windows the path cannot start with '~': `{}`",
            path.display()
        ));
    }

    if !path.is_dir() {
        return Err(anyhow!(
            "Path does not exist or is not a directory: `{}`",
            path.display()
        ));
    }

    let path = path.absolutize().context("Failed to absolutize the path")?;
    Ok(path)
}
