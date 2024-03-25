use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use itertools::Itertools;

use crate::cli::ENV_DEFAULT_VERSION;
use crate::cli::ENV_EDITOR_DIR;
use crate::unity::Version;

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

impl Version {
    pub(crate) fn is_editor_installed(self) -> Result<bool> {
        Ok(InstalledVersions::parent_dir()?
            .join(self.to_string())
            .exists())
    }

    /// Returns the path to the editor executable.
    pub(crate) fn editor_executable_path(self) -> Result<PathBuf> {
        let exe_path = InstalledVersions::parent_dir()?
            .join(self.to_string())
            .join(UNITY_EDITOR_EXE);
        if exe_path.exists() {
            Ok(exe_path)
        } else {
            Err(anyhow!("Unity version is not installed: {self}"))
        }
    }
}

/// A non-empty list of installed Unity versions, sorted from the oldest to the newest.
pub(crate) struct InstalledVersions {
    versions: Vec<Version>,
}

#[allow(clippy::from_over_into)]
impl Into<Vec<Version>> for InstalledVersions {
    fn into(self) -> Vec<Version> {
        self.versions
    }
}

impl AsRef<Vec<Version>> for InstalledVersions {
    fn as_ref(&self) -> &Vec<Version> {
        &self.versions
    }
}

impl InstalledVersions {
    /// Returns the parent directory of the editor installations and the list of installed versions.
    pub(crate) fn get<'a>() -> Result<(Cow<'a, Path>, Self)> {
        let parent_dir = Self::parent_dir()?;
        let installed_versions = InstalledVersions::from_dir(&parent_dir)?;
        Ok((parent_dir, installed_versions))
    }

    /// Returns a sorted list of installed Unity versions from the given directory or an error if no versions are found.
    fn from_dir<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let versions = fs::read_dir(&dir)
            .with_context(|| {
                format!(
                    "Cannot read available Unity editors in `{}`",
                    dir.as_ref().display()
                )
            })?
            .flatten()
            .map(|de| de.path()) //
            .filter(|p| p.is_dir() && p.join(UNITY_EDITOR_EXE).exists())
            .filter_map(|p| p.file_name()?.to_string_lossy().parse::<Version>().ok())
            .sorted_unstable()
            .collect_vec();

        if versions.is_empty() {
            Err(anyhow!(
                "No Unity installations found in `{}`",
                dir.as_ref().display()
            ))
        } else {
            Ok(Self { versions })
        }
    }

    /// Returns the list with only the versions that match the partial version or Err if there is no matching version.
    pub(crate) fn prune(self, partial_version: Option<&str>) -> Result<Self> {
        let Some(partial_version) = partial_version else {
            // No version to match, return the full list again.
            return Ok(self);
        };

        let versions = self
            .versions
            .into_iter()
            .filter(|v| v.to_string().starts_with(partial_version))
            .collect_vec();

        if versions.is_empty() {
            Err(anyhow!(
                "No Unity installation was found that matches version `{partial_version}`."
            ))
        } else {
            Ok(Self { versions })
        }
    }

    /// Returns the default version ucom uses for new Unity projects.
    pub(crate) fn default_version(&self) -> Result<Version> {
        let default_version = env::var_os(ENV_DEFAULT_VERSION)
            .and_then(|env| {
                self.versions
                    .iter()
                    .rev()
                    .find(|v| v.to_string().starts_with(env.to_string_lossy().as_ref()))
            })
            .or_else(|| self.versions.last())
            .ok_or_else(|| anyhow!("No Unity versions installed"))?;
        Ok(*default_version)
    }

    /// Returns the parent directory of the editor installations.
    fn parent_dir<'a>() -> Result<Cow<'a, Path>> {
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

    /// Returns the version of the latest-installed version that matches the partial version.
    pub(crate) fn latest(partial_version: Option<&str>) -> Result<Version> {
        let version = *InstalledVersions::from_dir(Self::parent_dir()?)?
            .prune(partial_version)?
            .as_ref()
            .last()
            .unwrap();
        Ok(version)
    }
}
