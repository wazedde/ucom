use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::ops::Deref;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use itertools::Itertools;
use serde::Deserialize;
use walkdir::{DirEntry, IntoIter, WalkDir};

use crate::unity::Version;
use crate::utils;

const VERSION_SUB_PATH: &str = "ProjectSettings/ProjectVersion.txt";

/// Returns an iterator over all directories in and including `root` that are not hidden.
pub fn walk_visible_directories(
    root: impl AsRef<Path>,
    max_depth: usize,
) -> walkdir::FilterEntry<IntoIter, fn(&DirEntry) -> bool> {
    WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|de| de.file_type().is_dir() && !is_hidden_directory(de))
}

fn is_hidden_directory(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| s.starts_with('.'))
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct Manifest {
    pub dependencies: BTreeMap<String, String>,
    #[serde(rename = "enableLockFile")]
    pub enable_lock_file: Option<bool>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct PackagesLock {
    pub dependencies: BTreeMap<String, String>,
}

impl PackagesLock {
    #[allow(dead_code)]
    pub fn from_project(project_dir: &Path) -> anyhow::Result<Self> {
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
    pub const fn to_short_str(self) -> &'static str {
        match self {
            Self::Local => "L",
            Self::Embedded => "E",
            Self::Git => "G",
            Self::LocalTarball => "T",
            Self::Registry => "R",
            Self::Builtin => "B",
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
    pub fn from_project(project: &ProjectPath) -> anyhow::Result<PackagesAvailability> {
        const MANIFEST_FILE: &str = "Packages/manifest.json";
        const PACKAGES_LOCK_FILE: &str = "Packages/packages-lock.json";

        let manifest_path = project.join(MANIFEST_FILE);

        if !manifest_path.exists() {
            return Ok(PackagesAvailability::NoManifest);
        }

        let file = File::open(manifest_path)?;
        let manifest: Manifest = serde_json::from_reader(BufReader::new(file))?;
        if manifest.enable_lock_file == Some(false) {
            // TODO: Read packages from Library/PackageManager/ProjectCache
            return Ok(PackagesAvailability::LockFileDisabled);
        }

        let lock_file_path = project.join(PACKAGES_LOCK_FILE);
        if !lock_file_path.exists() {
            return Ok(PackagesAvailability::NoLockFile);
        }

        let file = File::open(lock_file_path)?;
        let packages = serde_json::from_reader(BufReader::new(file))?;
        Ok(PackagesAvailability::Packages(packages))
    }
}

#[derive(Debug)]
pub struct ProjectSettings {
    pub product_name: String,
    pub company_name: String,
    pub bundle_version: String,
}

impl ProjectSettings {
    pub fn from_project(project: &ProjectPath) -> anyhow::Result<Self> {
        const SETTINGS_FILE: &str = "ProjectSettings/ProjectSettings.asset";
        let file = File::open(project.join(SETTINGS_FILE))?;
        Self::from_reader(BufReader::new(file))
    }

    /// Reads the project settings from a reader.
    /// Uses basic, hand rolled, parsing because ProjectSettings.asset
    /// is non-standard yaml that isn't fully supported by yaml crates.
    fn from_reader<R: Read + BufRead>(reader: R) -> anyhow::Result<Self> {
        let mut product_name: Option<_> = None;
        let mut company_name: Option<_> = None;
        let mut bundle_version: Option<_> = None;

        for line in reader.lines() {
            let line = line?;
            if product_name.is_none() {
                product_name = Self::try_parse_value("productName", &line).map(str::to_string);
            }

            if company_name.is_none() {
                company_name = Self::try_parse_value("companyName", &line).map(str::to_string);
            }

            if bundle_version.is_none() {
                bundle_version = Self::try_parse_value("bundleVersion", &line).map(str::to_string);
            }

            if product_name.is_some() && company_name.is_some() && bundle_version.is_some() {
                break;
            }
        }

        match (product_name, company_name, bundle_version) {
            (Some(product_name), Some(company_name), Some(bundle_version)) => {
                let setting = Self {
                    product_name,
                    company_name,
                    bundle_version,
                };
                Ok(setting)
            }
            _ => Err(anyhow!(
                "Could not find `productName` or `companyName` or `bundleVersion` in `ProjectSettings/ProjectSettings.asset`"
            )),
        }
    }

    /// Parse the given line as a key-value pair.
    fn try_parse_value<'a>(key: &str, line: &'a str) -> Option<&'a str> {
        line.split_once(':')
            .and_then(|(l, r)| l.trim().eq(key).then_some(r).map(str::trim))
    }
}

#[test]
fn test_project_settings_deserialization() {
    let data = include_str!("test_data/ProjectSettings.asset");
    let settings = ProjectSettings::from_reader(BufReader::new(data.as_bytes())).unwrap();
    assert_eq!(settings.product_name, "WebTest");
    assert_eq!(settings.company_name, "DefaultCompany");
    assert_eq!(settings.bundle_version, "0.1");
}

/// Represents a valid path to a Unity project.
pub struct ProjectPath(PathBuf);

impl Deref for ProjectPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.as_path()
    }
}

impl AsRef<Path> for ProjectPath {
    fn as_ref(&self) -> &Path {
        self.0.deref()
    }
}

impl ProjectPath {
    /// Creates a new `ProjectPath` from the given directory.
    /// Fails if the directory does not contain a Unity project.
    pub fn try_from(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = utils::resolve_absolute_dir_path(&path)?;
        if Self::contains_unity_project(&path) {
            Ok(Self(path.as_ref().to_path_buf()))
        } else {
            Err(anyhow!(
                "Path does not contain a Unity project: {}",
                path.display()
            ))
        }
    }

    /// Returns the Unity version for the project in the given directory.
    pub fn unity_version(&self) -> anyhow::Result<Version> {
        let version_file = self.join(VERSION_SUB_PATH);
        let mut reader = BufReader::new(File::open(&version_file)?);

        // ProjectVersion.txt looks like this:
        // m_EditorVersion: 2021.3.9f1
        // m_EditorVersionWithRevision: 2021.3.9f1 (ad3870b89536)

        let mut line = String::new();
        // Read the 1st line.
        _ = reader.read_line(&mut line)?;

        line.split_once(':')
            .filter(|(k, _)| k.trim() == "m_EditorVersion")
            .and_then(|(_, v)| v.trim().parse().ok())
            .ok_or_else(|| {
                anyhow!(
                    "Could not get project version from `{}`",
                    version_file.display()
                )
            })
    }

    /// Checks if the project directory has an `Assets` directory.
    pub fn ensure_assets_directory_exists(&self) -> anyhow::Result<()> {
        if self.join("Assets").exists() {
            Ok(())
        } else {
            Err(anyhow!(
                "Unity project does not have an `Assets` directory: `{}`",
                self.display()
            ))
        }
    }

    /// Checks the project for build profiles.
    /// TODO: Try to find profiles outside of the default `Assets/Settings/Build Profiles` directory.
    pub fn build_profiles(&self, version: Version) -> anyhow::Result<BuildProfilesStatus> {
        if version.major < 6000 {
            // Build profiles are supported in Unity 6.0 and later.
            return Ok(BuildProfilesStatus::NotSupported);
        }

        const PROFILES_PATH: &str = "Assets/Settings/Build Profiles";
        let path = self.join(PROFILES_PATH);
        if !path.exists() {
            return Ok(BuildProfilesStatus::NotFound);
        }

        let profiles = path
            .read_dir()?
            .filter_map(Result::ok)
            .filter(|de| de.file_type().is_ok_and(|ft| ft.is_file()))
            .filter(|de| de.path().extension().and_then(|ext| ext.to_str()) == Some("asset"))
            .map(|de| Path::new(PROFILES_PATH).join(de.file_name()))
            .sorted_by_key(|p| p.to_string_lossy().to_lowercase())
            .collect_vec();

        if profiles.is_empty() {
            Ok(BuildProfilesStatus::NotFound)
        } else {
            Ok(BuildProfilesStatus::Available(profiles))
        }
    }

    /// Checks if the directory contains a Unity project.
    fn contains_unity_project(dir: impl AsRef<Path>) -> bool {
        dir.as_ref().join(VERSION_SUB_PATH).exists()
    }
}

pub enum BuildProfilesStatus {
    /// Unity version does not support build profiles.
    NotSupported,
    /// No build profiles found.
    NotFound,
    /// Build profiles found.
    Available(Vec<PathBuf>),
}
