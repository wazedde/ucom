use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use serde::Deserialize;
use walkdir::{DirEntry, IntoIter, WalkDir};

use crate::cli::ENV_EDITOR_DIR;
use crate::unity::{ProjectPath, Version};

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

/// Returns the parent directory of the editor installations.
pub fn editor_parent_dir<'a>() -> Result<Cow<'a, Path>> {
    // Try to get the directory from the environment variable.
    env::var_os(ENV_EDITOR_DIR).map_or_else(
        || {
            // Use the default directory.
            let path = Path::new(UNITY_EDITOR_DIR);
            // If the default directory does not exist, return an error.
            path.exists().then(|| path.into()).ok_or_else(|| {
                let path = path.display();
                anyhow!(
                    "Set `{ENV_EDITOR_DIR}` to the editor directory, the default directory does not exist: `{path}`"
                )
            })
        },
        |path| {
            // Use the directory set by the environment variable.
            let path = Path::new(&path);
            // If the directory does not exist or is not a directory, return an error.
            (path.exists() && path.is_dir())
                .then(|| path.to_owned().into())
                .ok_or_else(|| {
                    let path = path.display();
                    anyhow!(
                        "Editor directory set by `{ENV_EDITOR_DIR}` is not a valid directory: `{path}`"
                    )
                })
        },
    )
}

/// Returns the list with only the versions that match the partial version or Err if there is no matching version.
pub fn matching_versions(
    versions: Vec<Version>,
    partial_version: Option<&str>,
) -> Result<Vec<Version>> {
    let Some(partial_version) = partial_version else {
        // No version to match, return the full list again.
        return Ok(versions);
    };

    let versions = versions
        .into_iter()
        .filter(|v| v.to_string().starts_with(partial_version))
        .collect_vec();

    if versions.is_empty() {
        Err(anyhow!(
            "No Unity installation was found that matches version `{partial_version}`."
        ))
    } else {
        Ok(versions)
    }
}

/// Returns version of the latest installed version that matches the partial version.
pub fn latest_installed_version(partial_version: Option<&str>) -> Result<Version> {
    let parent_dir = editor_parent_dir()?;
    let version = *matching_versions(available_unity_versions(parent_dir)?, partial_version)?
        .last()
        .expect("There should be at least one entry");

    Ok(version)
}

impl Version {
    pub fn is_editor_installed(self) -> Result<bool> {
        Ok(editor_parent_dir()?.join(self.to_string()).exists())
    }

    /// Returns the path to the editor executable.
    pub fn editor_executable_path(self) -> Result<PathBuf> {
        let exe_path = editor_parent_dir()?
            .join(self.to_string())
            .join(UNITY_EDITOR_EXE);

        if exe_path.exists() {
            Ok(exe_path)
        } else {
            Err(anyhow!("Unity version is not installed: {self}"))
        }
    }
}

/// Returns a list of available Unity versions sorted from the oldest to the newest.
pub fn available_unity_versions<P: AsRef<Path>>(install_dir: P) -> Result<Vec<Version>> {
    let err_ctx = || {
        format!(
            "Cannot read available Unity editors in `{}`",
            install_dir.as_ref().display()
        )
    };

    let versions = fs::read_dir(&install_dir)
        .with_context(err_ctx)?
        .flatten()
        .map(|de| de.path()) //
        .filter(|p| p.is_dir() && p.join(UNITY_EDITOR_EXE).exists())
        .filter_map(|p| p.file_name()?.to_string_lossy().parse::<Version>().ok())
        .sorted_unstable()
        .collect_vec();

    if versions.is_empty() {
        Err(anyhow!(
            "No Unity installations found in `{}`",
            install_dir.as_ref().display()
        ))
    } else {
        Ok(versions)
    }
}

pub fn recursive_dir_iter<P: AsRef<Path>>(
    root: P,
) -> walkdir::FilterEntry<IntoIter, fn(&DirEntry) -> bool> {
    WalkDir::new(root)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| e.file_type().is_dir() && !is_hidden(e))
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| s.starts_with('.'))
}

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub dependencies: BTreeMap<String, String>,
    #[serde(rename = "enableLockFile")]
    pub enable_lock_file: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct PackagesLock {
    pub dependencies: BTreeMap<String, String>,
}

#[allow(dead_code)]
impl PackagesLock {
    pub fn from_project(project_dir: &Path) -> Result<Self> {
        let file = File::open(project_dir.join("Packages/manifest.json"))?;
        serde_json::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct PackageInfo {
    pub version: String,
    pub depth: u32,
    pub source: Option<PackageSource>,
    pub dependencies: BTreeMap<String, String>,
    pub url: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PackageSource {
    Local,
    Embedded,
    Git,
    #[serde(rename = "local-tarball")]
    LocalTarball,
    Registry,
    Builtin,
}

impl PackageSource {
    pub fn to_short_str(self) -> &'static str {
        match self {
            PackageSource::Local => "L",
            PackageSource::Embedded => "E",
            PackageSource::Git => "G",
            PackageSource::LocalTarball => "T",
            PackageSource::Registry => "R",
            PackageSource::Builtin => "B",
        }
    }
}

#[derive(Deserialize, Debug, Default, PartialEq, Eq)]
pub struct Packages {
    pub dependencies: BTreeMap<String, PackageInfo>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum PackagesAvailability {
    /// The project does not have a manifest file.
    NoManifest,
    /// The project has a manifest file but the lock file is disabled.
    LockFileDisabled,
    /// The project has a manifest file and the lock file is enabled.
    Packages(Packages),
    /// The project has no lock file.
    NoLockFile,
}

impl Packages {
    pub fn from_project(project: &ProjectPath) -> Result<PackagesAvailability> {
        const MANIFEST_FILE: &str = "Packages/manifest.json";
        const PACKAGES_LOCK_FILE: &str = "Packages/packages-lock.json";

        let project_dir = project.as_path();
        let manifest_path = project_dir.join(MANIFEST_FILE);

        if !manifest_path.exists() {
            return Ok(PackagesAvailability::NoManifest);
        }

        let file = File::open(manifest_path)?;
        let manifest: Manifest = serde_json::from_reader(BufReader::new(file))?;
        if manifest.enable_lock_file == Some(false) {
            // TODO: Read packages from Library/PackageManager/ProjectCache
            return Ok(PackagesAvailability::LockFileDisabled);
        }

        let lock_file_path = project_dir.join(PACKAGES_LOCK_FILE);
        if !lock_file_path.exists() {
            return Ok(PackagesAvailability::NoLockFile);
        }

        let file = File::open(lock_file_path)?;
        let packages = serde_json::from_reader(BufReader::new(file))?;
        Ok(PackagesAvailability::Packages(packages))
    }
}

#[derive(Deserialize, Debug)]
pub struct Settings {
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
    pub build_number: Option<BTreeMap<String, String>>,

    #[serde(rename = "AndroidBundleVersionCode")]
    pub android_bundle_version_code: Option<String>,
}

impl Settings {
    pub fn from_project(project: &ProjectPath) -> Result<Self> {
        let project_dir = project.as_path();
        let file = File::open(project_dir.join("ProjectSettings/ProjectSettings.asset"))?;
        serde_yaml::from_reader(BufReader::new(file))
            .context("Error reading `ProjectSettings/ProjectSettings.asset`")
            .map_err(Into::into)
    }
}
