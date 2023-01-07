use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use path_absolutize::Absolutize;

use crate::cli::ENV_EDITOR_DIR;

/// Sub path to the executable on macOS.
#[cfg(target_os = "macos")]
const UNITY_EDITOR_EXE: &str = "Unity.app/Contents/MacOS/Unity";

/// Sub path to the executable on Windows.
#[cfg(target_os = "windows")]
const UNITY_EDITOR_EXE: &str = r"Editor\Unity.exe";

/// Other target platforms are not supported.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const UNITY_EDITOR_EXE: &str = compile_error!("Unsupported platform");

/// Parent directory of editor installations on macOS.
#[cfg(target_os = "macos")]
const UNITY_EDITOR_DIR: &str = "/Applications/Unity/Hub/Editor/";

/// Parent directory of editor installations on Windows.
#[cfg(target_os = "windows")]
const UNITY_EDITOR_DIR: &str = r"C:\Program Files\Unity\Hub\Editor";

/// Other target platforms are not supported.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const UNITY_EDITOR_DIR: &str = compile_error!("Unsupported platform");

/// Returns the Unity version used for the project.
pub(crate) fn version_used_by_project<P: AsRef<Path>>(project_dir: &P) -> Result<String> {
    const PROJECT_VERSION_FILE: &str = "ProjectSettings/ProjectVersion.txt";
    let version_file = project_dir.as_ref().join(PROJECT_VERSION_FILE);

    if !version_file.exists() {
        return Err(anyhow!(
            "Could not find Unity project in `{}`",
            project_dir.as_ref().to_string_lossy()
        ));
    }

    let mut reader = BufReader::new(File::open(&version_file)?);

    // ProjectVersion.txt looks like this:
    // m_EditorVersion: 2021.3.9f1
    // m_EditorVersionWithRevision: 2021.3.9f1 (ad3870b89536)

    let mut line = String::new();
    // Read the 1st line.
    let _ = reader.read_line(&mut line)?;

    line.starts_with("m_EditorVersion:")
        .then_some(line)
        .and_then(|l| {
            l.split(':') // Split the line,
                .nth(1) // and return 2nd element.
                .map(|version| version.trim().to_owned()) // Clean it up.
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not get project version from `{}`",
                version_file.to_string_lossy()
            )
        })
}

/// Returns the parent directory of the editor installations.
pub(crate) fn editor_parent_dir<'a>() -> Result<Cow<'a, Path>> {
    match env::var_os(ENV_EDITOR_DIR) {
        Some(path) => {
            let path = Path::new(&path);
            (path.exists() && path.is_dir())
                .then(|| path.to_owned().into())
                .ok_or_else(|| {
                    anyhow!(
                        "Editor directory set by `{}` is not a valid directory: `{}`",
                        ENV_EDITOR_DIR,
                        path.to_string_lossy()
                    )
                })
        }
        None => {
            let path = Path::new(UNITY_EDITOR_DIR);
            path.exists().then(|| path.into()).ok_or_else(|| {
                anyhow!(
                    "Set `{}` to the editor directory, the default directory does not exist: `{}`",
                    ENV_EDITOR_DIR,
                    path.to_string_lossy()
                )
            })
        }
    }
}

/// Returns the list with only the versions that match the partial version or Err if there is no matching version.
pub(crate) fn matching_versions(
    versions: Vec<OsString>,
    partial_version: Option<&str>,
) -> Result<Vec<OsString>> {
    let Some(partial_version) = partial_version else {
        // No version to match, return the full list again.
        return Ok(versions);
    };

    let versions: Vec<_> = versions
        .into_iter()
        .filter(|v| v.to_string_lossy().starts_with(partial_version))
        .collect();

    if !versions.is_empty() {
        Ok(versions)
    } else {
        Err(anyhow!(
            "No Unity installation was found that matches version `{}`.",
            partial_version
        ))
    }
}

/// Returns version and path to the editor app of the latest installed version that matches the partial version.
pub(crate) fn matching_editor(partial_version: Option<&str>) -> Result<(OsString, PathBuf)> {
    let parent_dir = editor_parent_dir()?;

    let version = matching_versions(available_unity_versions(&parent_dir)?, partial_version)?
        .last()
        .expect("There should be at least one entry")
        .to_owned();

    let editor_exe = parent_dir.join(&version).join(UNITY_EDITOR_EXE);
    Ok((version, editor_exe))
}

/// Returns version used by the project and the path to the editor.
pub(crate) fn matching_editor_used_by_project<P: AsRef<Path>>(
    project_dir: &P,
) -> Result<(OsString, PathBuf)> {
    let version = version_used_by_project(project_dir)?;

    // Check if that Unity version is installed.
    let editor_dir = editor_parent_dir()?.join(&version);

    if editor_dir.exists() {
        Ok((version.into(), editor_dir.join(UNITY_EDITOR_EXE)))
    } else {
        Err(anyhow!(
            "Unity version that the project uses is not installed: {}",
            version
        ))
    }
}

/// Returns a natural sorted list of available Unity versions.
pub(crate) fn available_unity_versions<P: AsRef<Path>>(install_dir: &P) -> Result<Vec<OsString>> {
    let mut versions: Vec<_> = fs::read_dir(install_dir)
        .with_context(|| {
            format!(
                "Cannot read available Unity editors in `{}`",
                install_dir.as_ref().to_string_lossy()
            )
        })?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| p.join(UNITY_EDITOR_EXE).exists())
        .flat_map(|p| p.file_name().map(|version| version.to_owned()))
        .collect();

    if !versions.is_empty() {
        versions.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
        Ok(versions)
    } else {
        Err(anyhow!(
            "No Unity installations found in `{}`",
            install_dir.as_ref().to_string_lossy()
        ))
    }
}

/// Returns validated absolute path to the project directory.
pub(crate) fn validate_project_path<P: AsRef<Path>>(project_dir: &P) -> Result<Cow<Path>> {
    let path = project_dir.as_ref();
    if cfg!(target_os = "windows") && path.starts_with("~") {
        return Err(anyhow!(
            "On Windows the path cannot start with '~': `{}`",
            path.to_string_lossy()
        ));
    }

    if !path.exists() {
        return Err(anyhow!(
            "Directory does not exists: `{}`",
            path.to_string_lossy()
        ));
    }

    if !path.is_dir() {
        return Err(anyhow!(
            "Path is not a directory: `{}`",
            path.to_string_lossy()
        ));
    }

    if path.has_root() {
        Ok(path.into())
    } else {
        Ok(path.absolutize()?)
    }
}
