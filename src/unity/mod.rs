pub use crate::unity::project::*;
pub use crate::unity::releases::*;
pub use crate::unity::version::*;
pub use crate::utils::spawn_cmd::*;

use anyhow::Result;
use sysinfo::System;

pub mod installations;
pub mod project;
pub mod release_api;
pub mod release_api_data;
pub mod releases;
pub mod version;

/// Checks if Unity Editor is running for the given project.
///
/// Returns `true` if a Unity Editor process is found with the project loaded.
/// Unity Hub processes are ignored as they may also reference project paths.
pub fn is_unity_editor_running(project: &ProjectPath) -> Result<bool> {
    let sys = System::new_all();
    let project_path = project.to_string_lossy();

    for process in sys.processes().values() {
        let cmd_args = process.cmd();

        // Skip Unity Hub - it may have -projectPath in its command line
        if cmd_args
            .iter()
            .any(|arg| arg.to_string_lossy().contains("Unity Hub"))
        {
            continue;
        }

        // Skip processes that don't contain the project path
        if !cmd_args
            .iter()
            .any(|arg| arg.to_string_lossy().contains(project_path.as_ref()))
        {
            continue;
        }

        // Check if the process has -projectPath in its command line
        if cmd_args.iter().any(|arg| {
            arg.to_string_lossy()
                .to_lowercase()
                .contains("-projectpath")
        }) {
            return Ok(true);
        }
    }

    Ok(false)
}
