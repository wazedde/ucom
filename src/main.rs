use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use clap::Parser;

use crate::args::Args;
use crate::cmd::CmdRunner;

mod args;
mod cmd;

fn main() -> Result<()> {
    let args = Args::parse();

    match args.action {
        args::Action::List { version_pattern } => {
            show_list(version_pattern).context("Cannot list installations")
        }

        args::Action::Run {
            version_pattern,
            wait,
            quiet,
            dry_run,
            args,
        } => run_unity_cmd(version_pattern, args)
            .context("Cannot run Unity")?
            .run(wait, quiet, dry_run),

        args::Action::New {
            version_pattern,
            path,
            wait,
            quiet,
            dry_run,
            args,
        } => new_project_cmd(version_pattern, path, args)
            .context("Cannot create project")?
            .run(wait, quiet, dry_run),

        args::Action::Open {
            path,
            version_pattern,
            wait,
            quiet,
            dry_run,
            args,
        } => open_project_cmd(&path, version_pattern, args)
            .context("Cannot open project")?
            .run(wait, quiet, dry_run),
    }
}

/// Lists installed Unity versions.
fn show_list(version_pattern: Option<String>) -> Result<()> {
    let root_path = installation_root_path();
    let versions = installed_matching_unity_versions(&version_pattern, &root_path);

    let Some(versions) = versions else {
        return Err(anyhow!("No Unity installations found in: {}", root_path.to_string_lossy()));
    };

    println!("Installed Unity versions:");
    for editor in versions {
        println!("{}", editor.to_string_lossy());
    }

    Ok(())
}

/// Returns command that runs Unity.
fn run_unity_cmd(version_pattern: Option<String>, unity_args: Option<Vec<String>>) -> Result<CmdRunner> {
    let (unity_version, directory) = find_latest_matching_unity(&version_pattern)?;

    let mut cmd = Command::new(unity_executable_path(&directory));
    cmd.args(unity_args.unwrap_or_default());

    Ok(CmdRunner::new(
        cmd,
        format!("Running Unity {}", unity_version.to_string_lossy()))
    )
}

/// Returns command that creates an empty project at the given path.
fn new_project_cmd(
    version_pattern: Option<String>,
    project_path: PathBuf,
    unity_args: Option<Vec<String>>,
) -> Result<CmdRunner> {
    // Check if destination already exists.
    if project_path.exists() {
        return Err(anyhow!(
            "Directory already exists: {}",
            project_path.canonicalize()?.to_string_lossy()
        ));
    }

    let (version, unity_directory) = find_latest_matching_unity(&version_pattern)?;
    let project_path: String = project_path.to_string_lossy().into();

    let mut cmd = Command::new(unity_executable_path(&unity_directory));
    cmd.args(["-createProject", &project_path])
        .args(unity_args.unwrap_or_default());

    Ok(CmdRunner::new(
        cmd,
        format!(
            "Creating Unity {} project in {}",
            version.to_string_lossy(),
            project_path
        ))
    )
}

/// Returns command that opens the project at the given path.
fn open_project_cmd(
    project_path: &Path,
    version_pattern: Option<String>,
    unity_args: Option<Vec<String>>,
) -> Result<CmdRunner> {
    // Make sure the project path exists.
    let Ok(project_path) = &project_path.canonicalize() else {
        return Err(anyhow!(
            "Directory does not exist: {}",
            project_path.to_string_lossy()
        ));
    };

    let (version, unity_directory) = if version_pattern.is_some() {
        find_latest_matching_unity(&version_pattern)?
    } else {
        // Get the Unity version the project uses.
        let version: OsString = unity_project_version(project_path)?.into();
        // Check if that Unity version is installed.
        let directory = Path::new(&installation_root_path()).join(&version);

        if !directory.exists() {
            return Err(anyhow!(
                "Unity version that the project uses is not installed: {}",
                version.to_string_lossy()
            ));
        }
        (version, directory)
    };

    let unity_path = unity_executable_path(&unity_directory);

    // Build the command to execute.
    let mut cmd = Command::new(unity_path);
    cmd.args(["-projectPath", &project_path.to_string_lossy()])
        .args(unity_args.unwrap_or_default());

    Ok(CmdRunner::new(
        cmd,
        format!(
            "Opening Unity {} project in {}",
            version.to_string_lossy(),
            project_path.to_string_lossy()
        ))
    )
}

const PROJECT_VERSION_FILE: &str = "ProjectSettings/ProjectVersion.txt";

/// Returns the Unity version used for the project.
fn unity_project_version(project_path: &Path) -> Result<String> {
    let file_path = project_path.join(PROJECT_VERSION_FILE);
    let project_version = fs::read_to_string(&file_path);

    let Ok(project_version) = project_version else {
        return Err(anyhow!(
            "Directory does not contain a Unity project: {}",
            project_path.to_string_lossy())
        );
    };

    // ProjectVersion.txt looks like this:
    // m_EditorVersion: 2021.3.9f1
    // m_EditorVersionWithRevision: 2021.3.9f1 (ad3870b89536)

    let project_version = project_version
        .lines()
        // Get the 1st line.
        .next()
        // Split that line and return 2nd element.
        .and_then(|line| line.split(':').nth(1))
        // Clean it up.
        .map(|version| version.trim());

    let Some(project_version) = project_version else {
        return Err(anyhow!(
            "Could not get project version from: {}",
            file_path.to_string_lossy())
        );
    };

    Ok(project_version.to_string())
}

/// Returns the root path of the installations.
fn installation_root_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        "/Applications/Unity/Hub/Editor/".to_string().into()
    } else if cfg!(target_os = "windows") {
        todo!()
    } else {
        unimplemented!()
    }
}

/// Returns the path to the executable.
fn unity_executable_path(editor_path: &PathBuf) -> PathBuf {
    if cfg!(target_os = "macos") {
        return Path::new(&editor_path).join("Unity.app/Contents/MacOS/Unity");
    } else if cfg!(target_os = "windows") {
        todo!()
    } else {
        unimplemented!()
    }
}

/// Returns the latest installed version that matches the pattern.
fn find_latest_matching_unity_version(version_pattern: &Option<String>) -> Option<OsString> {
    let root_path = installation_root_path();
    let matching_versions = installed_matching_unity_versions(version_pattern, &root_path)?;
    Some(matching_versions.last()?.to_os_string())
}

/// Returns list of installed versions that match the pattern.
fn installed_matching_unity_versions(
    version_pattern: &Option<String>,
    root_path: &Path,
) -> Option<Vec<OsString>> {
    let installed_versions = installed_unity_versions(root_path)?;

    let Some(pattern) = version_pattern else {
        return Some(installed_versions);
    };

    let matching_versions: Vec<_> = installed_versions
        .into_iter()
        .filter(|v| v.to_string_lossy().starts_with(pattern))
        .collect();

    if matching_versions.is_empty() {
        None
    } else {
        Some(matching_versions)
    }
}

/// Returns version and directory of the latest installed version that matches the pattern.
fn find_latest_matching_unity(version_pattern: &Option<String>) -> Result<(OsString, PathBuf)> {
    let Some(found_version) = find_latest_matching_unity_version(version_pattern) else {
        return Err(anyhow!(
            "No Unity installation was found that matches version: {}",
            version_pattern.clone().unwrap_or_else(|| "<any>".into()))
        );
    };

    let root_path = installation_root_path();
    let full_path = Path::new(&root_path).join(&found_version);
    Ok((found_version, full_path))
}

/// Returns a natural sorted list of installed Unity versions.
fn installed_unity_versions(search_path: &Path) -> Option<Vec<OsString>> {
    let mut versions: Vec<_> = fs::read_dir(search_path)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .flat_map(|path| path.file_name().map(|f| f.to_os_string()))
        .collect();

    if versions.is_empty() {
        None
    } else {
        versions.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
        Some(versions)
    }
}
