use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use serde::Deserialize;
use walkdir::{DirEntry, IntoIter, WalkDir};

use crate::unity;
use crate::unity::Version;

const VERSION_SUB_PATH: &str = "ProjectSettings/ProjectVersion.txt";

/// Returns all directories in and including `root` that are not hidden.
pub(crate) fn directory_walker(
    root: impl AsRef<Path>,
) -> walkdir::FilterEntry<IntoIter, fn(&DirEntry) -> bool> {
    WalkDir::new(root)
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| e.file_type().is_dir() && !is_hidden_directory(e))
}

fn is_hidden_directory(entry: &DirEntry) -> bool {
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

impl PackagesLock {
    #[allow(dead_code)]
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

#[derive(Debug)]
pub(crate) struct ProjectSettings {
    pub(crate) product_name: String,
    pub(crate) company_name: String,
    pub(crate) bundle_version: String,
}

impl ProjectSettings {
    pub(crate) fn from_project(project: &ProjectPath) -> anyhow::Result<Self> {
        const SETTINGS_FILE: &str = "ProjectSettings/ProjectSettings.asset";
        let file = File::open(project.as_path().join(SETTINGS_FILE))?;
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
                Ok(ProjectSettings { product_name, company_name, bundle_version })
            }
            _ => Err(anyhow!("Could not find `productName` or `companyName` or `bundleVersion` in `ProjectSettings/ProjectSettings.asset`")),
        }
    }

    /// Parse the given line as a key-value pair.
    fn try_parse_value<'a>(key: &str, line: &'a str) -> Option<&'a str> {
        line.split_once(":")
            .and_then(|(l, r)| l.trim().eq(key).then_some(r).map(|r| r.trim()))
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
pub(crate) struct ProjectPath(PathBuf);

impl ProjectPath {
    /// Creates a new `ProjectPath` from the given directory.
    pub(crate) fn try_from(path: impl AsRef<Path>) -> anyhow::Result<Self> {
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
        if self.as_path().join("Assets").exists() {
            Ok(())
        } else {
            Err(anyhow!(
                "Unity project does not have an `Assets` directory: `{}`",
                self.as_path().display()
            ))
        }
    }

    /// Checks if the directory contains a Unity project.
    fn is_unity_project_directory(dir: impl AsRef<Path>) -> bool {
        dir.as_ref().join(VERSION_SUB_PATH).exists()
    }
}
