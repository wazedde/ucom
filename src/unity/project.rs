use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use serde::Deserialize;
use walkdir::{DirEntry, IntoIter, WalkDir};

use crate::unity;
use crate::unity::Version;

const VERSION_SUB_PATH: &str = "ProjectSettings/ProjectVersion.txt";

pub(crate) fn recursive_dir_iter<P: AsRef<Path>>(
    root: P,
) -> walkdir::FilterEntry<IntoIter, fn(&DirEntry) -> bool> {
    WalkDir::new(root)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| e.file_type().is_dir() && !is_hidden_file(e))
}

fn is_hidden_file(entry: &DirEntry) -> bool {
    match entry.file_name().to_str() {
        Some(s) => s.starts_with('.'),
        None => false,
    }
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub(crate) struct Manifest {
    pub(crate) dependencies: BTreeMap<String, String>,
    #[serde(rename = "enableLockFile")]
    pub(crate) enable_lock_file: Option<bool>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub(crate) struct PackagesLock {
    pub(crate) dependencies: BTreeMap<String, String>,
}

#[allow(dead_code)]
impl PackagesLock {
    pub(crate) fn from_project(project_dir: &Path) -> anyhow::Result<Self> {
        let file = File::open(project_dir.join("Packages/manifest.json"))?;
        serde_json::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub(crate) struct PackageInfo {
    pub(crate) version: String,
    pub(crate) depth: u32,
    pub(crate) source: Option<PackageSource>,
    pub(crate) dependencies: BTreeMap<String, String>,
    pub(crate) url: Option<String>,
}

#[derive(Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PackageSource {
    Local,
    Embedded,
    Git,
    #[serde(rename = "local-tarball")]
    LocalTarball,
    Registry,
    Builtin,
}

impl PackageSource {
    pub(crate) fn to_short_str(self) -> &'static str {
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
pub(crate) struct Packages {
    pub(crate) dependencies: BTreeMap<String, PackageInfo>,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum PackagesAvailability {
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
    pub(crate) fn from_project(project: &ProjectPath) -> anyhow::Result<PackagesAvailability> {
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
pub(crate) struct Settings {
    #[serde(rename = "PlayerSettings")]
    pub(crate) player_settings: PlayerSettings,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub(crate) struct PlayerSettings {
    #[serde(rename = "productName")]
    pub(crate) product_name: String,

    #[serde(rename = "companyName")]
    pub(crate) company_name: String,

    #[serde(rename = "bundleVersion")]
    pub(crate) bundle_version: String,

    #[serde(rename = "buildNumber")]
    pub(crate) build_number: Option<BTreeMap<String, String>>,

    #[serde(rename = "AndroidBundleVersionCode")]
    pub(crate) android_bundle_version_code: Option<String>,
}

impl Settings {
    pub(crate) fn from_project(project: &ProjectPath) -> anyhow::Result<Self> {
        let project_dir = project.as_path();
        let file = File::open(project_dir.join("ProjectSettings/ProjectSettings.asset"))?;
        serde_yml::from_reader(BufReader::new(file))
            .context("Error reading `ProjectSettings/ProjectSettings.asset`")
            .map_err(Into::into)
    }
}

/// Represents a valid path to a Unity project.
pub(crate) struct ProjectPath(PathBuf);

impl ProjectPath {
    /// Creates a new `ProjectPath` from the given directory.
    pub(crate) fn try_from<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = unity::to_absolute_dir_path(&path)?;
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
    pub(crate) fn as_path(&self) -> &Path {
        self.0.as_path()
    }

    /// Returns the Unity version for the project in the given directory.
    pub(crate) fn unity_version(&self) -> anyhow::Result<Version> {
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
    pub(crate) fn check_assets_directory_exists(&self) -> anyhow::Result<()> {
        let assets_path = self.as_path().join("Assets");

        if assets_path.exists() {
            Ok(())
        } else {
            Err(anyhow!(
                "Unity project does not have an `Assets` directory: `{}`",
                self.as_path().display()
            ))
        }
    }

    /// Checks if the directory contains a Unity project.
    fn is_unity_project_directory<P: AsRef<Path>>(dir: P) -> bool {
        dir.as_ref().join(VERSION_SUB_PATH).exists()
    }
}
