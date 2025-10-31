use std::process::Command;

use anyhow::{Context, Result};

use crate::unity::ProjectPath;

/// Checks if Unity editor is running for the given project.
///
/// This function uses platform-specific process detection to determine if a Unity editor
/// instance is currently running with the specified project loaded via `-projectPath`.
pub fn is_unity_editor_running(project: &ProjectPath) -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        is_unity_running_macos(project)
    }

    #[cfg(target_os = "windows")]
    {
        is_unity_running_windows(project)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(false)
    }
}

#[cfg(target_os = "macos")]
fn is_unity_running_macos(project: &ProjectPath) -> Result<bool> {
    let output = Command::new("ps")
        .args(["ax", "-o", "command"])
        .output()
        .context("Failed to execute ps command")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let project_path = project.to_string_lossy();

    // Look for Unity process with -projectPath argument matching this project
    // Note: Unity Hub launches with lowercase -projectpath, direct launch uses -projectPath
    for line in stdout.lines() {
        if line.contains("Unity")
            && (line.contains("-projectPath") || line.contains("-projectpath"))
            && line.contains(&*project_path)
        {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(target_os = "windows")]
fn is_unity_running_windows(project: &ProjectPath) -> Result<bool> {
    let output = Command::new("wmic")
        .args(["process", "where", "name='Unity.exe'", "get", "CommandLine"])
        .output()
        .context("Failed to execute wmic command")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let project_path = project.to_string_lossy();

    // Look for Unity process with -projectPath argument matching this project
    // Note: Unity Hub launches with lowercase -projectpath, direct launch uses -projectPath
    for line in stdout.lines() {
        if (line.contains("-projectPath") || line.contains("-projectpath"))
            && line.contains(&*project_path)
        {
            return Ok(true);
        }
    }

    Ok(false)
}
