use crate::unity::{ProjectPath, Version};
use anyhow::Context;
use std::path::{Path, PathBuf};

/// Unified project setup for commands
pub struct ProjectSetup {
    pub project: ProjectPath,
    pub unity_version: Version,
}

impl ProjectSetup {
    /// Initialise project and validate Unity version
    pub fn new(project_dir: &Path) -> anyhow::Result<Self> {
        let project =
            ProjectPath::from_dir(project_dir).context("Failed to locate Unity project")?;
        let unity_version = project
            .unity_version()
            .context("Failed to read Unity version from project")?;

        Ok(Self {
            project,
            unity_version,
        })
    }

    /// Get the editor executable for this project's Unity version
    pub fn editor_executable(&self) -> anyhow::Result<PathBuf> {
        self.unity_version.editor_executable_path()
    }
}
