use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use path_absolutize::Absolutize;
use serde::Deserialize;

use crate::cli::ENV_EDITOR_DIR;
use crate::unity_version::UnityVersion;

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
pub fn version_used_by_project<P: AsRef<Path>>(project_dir: &P) -> Result<UnityVersion> {
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
                .map(str::trim)
                .and_then(|v| v.parse().ok())
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not get project version from `{}`",
                version_file.to_string_lossy()
            )
        })
}

/// Returns the parent directory of the editor installations.
pub fn editor_parent_dir<'a>() -> Result<Cow<'a, Path>> {
    env::var_os(ENV_EDITOR_DIR).map_or_else(
        || {
            let path = Path::new(UNITY_EDITOR_DIR);
            path.exists().then(|| path.into()).ok_or_else(|| {
                let path = path.to_string_lossy();
                anyhow!(
                    "Set `{ENV_EDITOR_DIR}` to the editor directory, the default directory does not exist: `{path}`"
                )
            })
        },
        |path| {
            let path = Path::new(&path);
            (path.exists() && path.is_dir())
                .then(|| path.to_owned().into())
                .ok_or_else(|| {
                    let path = path.to_string_lossy();
                    anyhow!(
                        "Editor directory set by `{ENV_EDITOR_DIR}` is not a valid directory: `{path}`"
                    )
                })
        },
    )
}

pub fn is_editor_installed(version: UnityVersion) -> Result<bool> {
    Ok(editor_parent_dir()?.join(version.to_string()).exists())
}

/// Returns the list with only the versions that match the partial version or Err if there is no matching version.
pub fn matching_versions(
    versions: Vec<UnityVersion>,
    partial_version: Option<&str>,
) -> Result<Vec<UnityVersion>> {
    let Some(partial_version) = partial_version else {
        // No version to match, return the full list again.
        return Ok(versions);
    };

    let versions: Vec<_> = versions
        .into_iter()
        .filter(|v| v.to_string().starts_with(partial_version))
        .collect();

    if versions.is_empty() {
        Err(anyhow!(
            "No Unity installation was found that matches version `{partial_version}`."
        ))
    } else {
        Ok(versions)
    }
}

/// Returns version and path to the editor app of the latest installed version that matches the partial version.
pub fn matching_editor(partial_version: Option<&str>) -> Result<(UnityVersion, PathBuf)> {
    let parent_dir = editor_parent_dir()?;
    let version = *matching_versions(available_unity_versions(&parent_dir)?, partial_version)?
        .last()
        .expect("There should be at least one entry");

    let editor_exe = parent_dir.join(version.to_string()).join(UNITY_EDITOR_EXE);
    Ok((version, editor_exe))
}

/// Returns version used by the project and the path to the editor.
pub fn matching_editor_used_by_project<P: AsRef<Path>>(
    project_dir: &P,
) -> Result<(UnityVersion, PathBuf)> {
    let version = version_used_by_project(project_dir)?;

    // Check if that Unity version is installed.
    let editor_dir = editor_parent_dir()?.join(version.to_string());
    if editor_dir.exists() {
        Ok((version, editor_dir.join(UNITY_EDITOR_EXE)))
    } else {
        Err(anyhow!(
            "Unity version that the project uses is not installed: {version}"
        ))
    }
}

/// Returns a natural sorted list of available Unity versions.
pub fn available_unity_versions<P: AsRef<Path>>(install_dir: &P) -> Result<Vec<UnityVersion>> {
    let mut versions: Vec<_> = fs::read_dir(install_dir)
        .with_context(|| {
            format!(
                "Cannot read available Unity editors in `{}`",
                install_dir.as_ref().to_string_lossy()
            )
        })?
        .flat_map(|r| r.map(|e| e.path()))
        .filter(|p| p.is_dir() && p.join(UNITY_EDITOR_EXE).exists())
        .filter_map(|p| p.file_name().map(std::borrow::ToOwned::to_owned))
        .filter_map(|version| version.to_string_lossy().parse::<UnityVersion>().ok())
        .collect();

    if versions.is_empty() {
        Err(anyhow!(
            "No Unity installations found in `{}`",
            install_dir.as_ref().to_string_lossy()
        ))
    } else {
        versions.sort();
        Ok(versions)
    }
}

/// Returns validated absolute path to the project directory.
pub fn validate_project_path<P: AsRef<Path>>(project_dir: &P) -> Result<Cow<Path>> {
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

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub dependencies: BTreeMap<String, String>,
}

impl Manifest {
    pub fn from_project(project_dir: &Path) -> Result<Self> {
        let file = File::open(project_dir.join("Packages/manifest.json"))?;
        serde_json::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}

#[derive(Deserialize, Debug)]
pub struct PackageInfo {
    pub version: String,
    pub depth: u32,
    pub source: String,
    pub dependencies: BTreeMap<String, String>,
    pub url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Packages {
    pub dependencies: BTreeMap<String, PackageInfo>,
}

impl Packages {
    pub fn from_project(project_dir: &Path) -> Result<Self> {
        let file = File::open(project_dir.join("Packages/packages-lock.json"))?;
        serde_json::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}

#[derive(Deserialize, Debug)]
pub struct ProjectSettings {
    #[serde(rename = "PlayerSettings")]
    pub player_settings: PlayerSettings,
}

#[derive(Deserialize, Debug)]
pub struct PlayerSettings {
    #[serde(rename = "productName")]
    pub product_name: String,

    #[serde(rename = "companyName")]
    pub company_name: String,

    #[serde(rename = "bundleVersion")]
    pub bundle_version: String,

    #[serde(rename = "buildNumber")]
    pub build_number: Option<HashMap<String, String>>,

    #[serde(rename = "AndroidBundleVersionCode")]
    pub android_bundle_version_code: Option<String>,
}

impl ProjectSettings {
    pub fn from_project(project_dir: &Path) -> Result<Self> {
        let file = File::open(project_dir.join("ProjectSettings/ProjectSettings.asset"))?;
        serde_yaml::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}
