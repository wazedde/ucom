use crate::cli::ENV_EDITOR_DIR;
use crate::unity::vec1::{Vec1, Vec1Err};
use crate::unity::Version;
use anyhow::{anyhow, Context};
use itertools::Itertools;
use std::borrow::Cow;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::{env, fs};

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

pub(crate) struct Installations {
    pub(crate) install_dir: PathBuf,
    pub(crate) versions: VersionList,
}

impl Installations {
    /// Returns a list of installed Unity versions or an error if no versions are found.
    pub(crate) fn find(version_prefix: Option<&str>) -> anyhow::Result<Installations> {
        let install_dir = Self::parent_dir()?.into_owned();
        let versions = VersionList::from_dir(&install_dir)?.filter_by_prefix(version_prefix)?;
        Ok(Installations {
            install_dir,
            versions,
        })
    }

    /// Returns a list of installed Unity versions or `None` if no versions are found.
    pub(crate) fn try_find(version_prefix: Option<&str>) -> Option<Installations> {
        Self::find(version_prefix).ok()
    }

    /// Returns the version of the latest-installed version that matches the given prefix.
    pub(crate) fn latest(version_prefix: Option<&str>) -> anyhow::Result<Version> {
        let version = *VersionList::from_dir(Self::parent_dir()?)?
            .filter_by_prefix(version_prefix)?
            .last();
        Ok(version)
    }

    /// Returns the parent directory of the editor installations.
    fn parent_dir<'a>() -> anyhow::Result<Cow<'a, Path>> {
        // Try to get the directory from the environment variable.
        if let Some(path) = env::var_os(ENV_EDITOR_DIR) {
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
        } else {
            // Use the default directory.
            let path = Path::new(UNITY_EDITOR_DIR);
            // If the default directory does not exist, return an error.
            path.exists().then(|| path.into()).ok_or_else(|| {
                let path = path.display();
                anyhow!(
                    "Set `{ENV_EDITOR_DIR}` to the editor directory, the default directory does not exist: `{path}`"
                )
            })
        }
    }
}

impl Version {
    pub(crate) fn is_editor_installed(self) -> anyhow::Result<bool> {
        Ok(Installations::parent_dir()?.join(self.to_string()).exists())
    }

    /// Returns the path to the editor executable.
    pub(crate) fn editor_executable_path(self) -> anyhow::Result<PathBuf> {
        let exe_path = Installations::parent_dir()?
            .join(self.to_string())
            .join(UNITY_EDITOR_EXE);
        if exe_path.exists() {
            Ok(exe_path)
        } else {
            Err(anyhow!("Unity version is not installed: {self}"))
        }
    }
}

/// A non-empty list of Unity versions, sorted from the oldest to the newest.
pub(crate) struct VersionList(Vec1<Version>);

impl Deref for VersionList {
    type Target = Vec1<Version>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Vec<Version>> for VersionList {
    type Error = Vec1Err;

    fn try_from(value: Vec<Version>) -> Result<Self, Self::Error> {
        Vec1::try_from(value).map(|mut versions| {
            versions.sort_unstable();
            Self(versions)
        })
    }
}

#[allow(dead_code)]
impl VersionList {
    pub(crate) fn into_vec(self) -> Vec<Version> {
        self.0.into()
    }

    /// Returns a sorted list of installed Unity versions from the given directory or an error if no versions are found.
    fn from_dir(dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let versions = fs::read_dir(&dir)
            .with_context(|| {
                format!(
                    "Cannot read available Unity editors in `{}`",
                    dir.as_ref().display()
                )
            })?
            .map_while(Result::ok)
            .map(|de| de.path()) //
            .filter(|p| p.is_dir() && p.join(UNITY_EDITOR_EXE).exists())
            .filter_map(|p| p.file_name()?.to_string_lossy().parse::<Version>().ok())
            .collect_vec();

        VersionList::try_from(versions).map_err(|_| {
            anyhow!(
                "No Unity installations found in `{}`",
                dir.as_ref().display()
            )
        })
    }

    pub(crate) fn filter_by_prefix(self, version_prefix: Option<&str>) -> anyhow::Result<Self> {
        let Some(version_prefix) = version_prefix else {
            // No version to match, return the full list again.
            return Ok(self);
        };

        let mut versions = self.into_vec();
        versions.retain(|v| v.to_string().starts_with(version_prefix));

        Vec1::try_from(versions).map(VersionList).map_err(|_| {
            anyhow!("No Unity installation was found that matches version `{version_prefix}`.")
        })
    }
}
