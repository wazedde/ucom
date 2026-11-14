use crate::unity::{Version, VersionPattern};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::vec1::{Vec1, Vec1Error};
use anyhow::{Context, anyhow, bail, ensure};
use itertools::Itertools;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::{env, fs};

const ENV_EDITOR_DIR: &str = "UCOM_EDITOR_DIR";

mod platform {
    use std::path::PathBuf;

    /// Sub path to the executable on macOS.
    #[cfg(target_os = "macos")]
    pub const UNITY_EDITOR_EXE: &str = "Unity.app/Contents/MacOS/Unity";

    /// Sub path to the executable on Windows.
    #[cfg(target_os = "windows")]
    pub const UNITY_EDITOR_EXE: &str = r"Editor\Unity.exe";

    /// Sub path to the executable on Linux.
    #[cfg(target_os = "linux")]
    pub const UNITY_EDITOR_EXE: &str = "Editor/Unity";

    /// Other target platforms are not supported.
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    pub const UNITY_EDITOR_EXE: &str = compile_error!("Unsupported platform");

    /// Returns the default parent directory of editor installations on macOS.
    #[cfg(target_os = "macos")]
    pub fn default_editor_dir() -> Option<PathBuf> {
        Some(PathBuf::from("/Applications/Unity/Hub/Editor/"))
    }

    /// Returns the default parent directory of editor installations on Windows.
    #[cfg(target_os = "windows")]
    pub fn default_editor_dir() -> Option<PathBuf> {
        Some(PathBuf::from(r"C:\Program Files\Unity\Hub\Editor"))
    }

    /// Returns the default parent directory of editor installations on Linux, or `None` if the
    /// home directory is unknown.
    #[cfg(target_os = "linux")]
    pub fn default_editor_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join("Unity/Hub/Editor"))
    }

    /// Other target platforms are not supported.
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    pub fn default_editor_dir() -> Option<PathBuf> {
        compile_error!("Unsupported platform")
    }
}

//
// SortedVersions
//

/// A non-empty list of Unity versions, sorted from the oldest to the newest.
pub struct SortedVersions(Vec1<Version>);

impl Deref for SortedVersions {
    type Target = Vec1<Version>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<Vec<Version>> for SortedVersions {
    type Error = Vec1Error;

    /// Converts a vector of versions into a sorted list of versions.
    fn try_from(versions: Vec<Version>) -> Result<Self, Self::Error> {
        Vec1::try_from(versions).map(|mut versions| {
            versions.sort_unstable();
            Self(versions)
        })
    }
}

impl SortedVersions {
    /// Converts the list into a [`Vec`].
    pub fn into_vec(self) -> Vec<Version> {
        self.0.into()
    }

    /// Returns a sorted list of installed Unity versions from the given directory or an error if no versions are found.
    fn from_dir(dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let versions = fs::read_dir(&dir)
            .with_context(|| {
                format!(
                    "Cannot read available Unity editors in `{}`",
                    dir.as_ref().normalized_display()
                )
            })?
            .flatten()
            .map(|de| de.path())
            .filter(|p| p.is_dir() && p.join(platform::UNITY_EDITOR_EXE).exists())
            .filter_map(|p| p.file_name()?.to_string_lossy().parse::<Version>().ok())
            .collect_vec();

        Self::try_from(versions).map_err(|_| {
            anyhow!(
                "No Unity installations found in `{}`",
                dir.as_ref().normalized_display()
            )
        })
    }

    pub fn filter_by_pattern(self, pattern: Option<&VersionPattern>) -> anyhow::Result<Self> {
        let Some(pattern) = pattern else {
            // No version to match, return the full list again.
            return Ok(self);
        };

        let mut versions = self.into_vec();
        versions.retain(|&v| pattern.matches(v));

        // No need to sort again, since the list is already sorted.
        Vec1::try_from(versions).map(Self).map_err(|_| {
            anyhow!("No Unity installation was found that matches version `{pattern}`.")
        })
    }
}

//
// Installations
//

/// The installed versions and the root directory they are installed in.
pub struct Installations {
    pub install_dir: PathBuf,
    pub versions: SortedVersions,
}

impl Installations {
    /// Returns a list of installed Unity versions or an error if no versions are found.
    pub fn find_installations(pattern: Option<&VersionPattern>) -> anyhow::Result<Self> {
        let install_dir = Self::editor_parent_dir()?.to_path_buf();
        let versions = SortedVersions::from_dir(&install_dir)?.filter_by_pattern(pattern)?;
        Ok(Self {
            install_dir,
            versions,
        })
    }

    /// Returns a list of installed Unity versions or `None` if no versions are found.
    pub fn try_find_installations(pattern: Option<&VersionPattern>) -> Option<Self> {
        Self::find_installations(pattern).ok()
    }

    /// Returns the latest installed version that matches the given pattern.
    pub fn latest_installed_version(pattern: Option<&VersionPattern>) -> anyhow::Result<Version> {
        let version = *SortedVersions::from_dir(Self::editor_parent_dir()?)?
            .filter_by_pattern(pattern)?
            .last();
        Ok(version)
    }

    /// Returns the parent directory of the editor installations.
    fn editor_parent_dir() -> anyhow::Result<&'static Path> {
        // The error is kept as text, since `anyhow::Error` cannot be shared from a static.
        static EDITOR_PARENT_DIR: LazyLock<Result<PathBuf, String>> = LazyLock::new(|| {
            Installations::resolve_unity_editor_directory().map_err(|e| format!("{e:#}"))
        });

        EDITOR_PARENT_DIR
            .as_deref()
            .map_err(|message| anyhow!("{message}"))
    }

    /// Resolves the Unity editor directory from the environment variable or the default path,
    /// and checks that it holds at least one installation.
    fn resolve_unity_editor_directory() -> anyhow::Result<PathBuf> {
        let Some(path) = env::var_os(ENV_EDITOR_DIR).map(PathBuf::from) else {
            let path = platform::default_editor_dir().with_context(|| {
                format!("Cannot find the home directory for the default editor directory; set `{ENV_EDITOR_DIR}`")
            })?;
            ensure!(
                Self::contains_installations(&path)?,
                "No Unity installations found in `{}`. Set `{ENV_EDITOR_DIR}` or install Unity.",
                path.normalized_display()
            );
            return Ok(path);
        };

        ensure!(
            path.is_dir(),
            "The editor directory set by `{ENV_EDITOR_DIR}` is not a directory: `{}`",
            path.normalized_display()
        );
        ensure!(
            Self::contains_installations(&path)?,
            "No Unity installations found in `{}`, the editor directory set by `{ENV_EDITOR_DIR}`.",
            path.normalized_display()
        );
        Ok(path)
    }

    /// Returns whether `dir` holds at least one Unity installation. A missing directory holds
    /// none; one that exists but cannot be read is an error.
    fn contains_installations(dir: &Path) -> anyhow::Result<bool> {
        if !dir.is_dir() {
            return Ok(false);
        }
        let has_installations = dir
            .read_dir()
            .with_context(|| {
                format!(
                    "Cannot read editor directory `{}`",
                    dir.normalized_display()
                )
            })?
            .filter_map(Result::ok)
            .any(|de| de.path().join(platform::UNITY_EDITOR_EXE).exists());
        Ok(has_installations)
    }
}

impl Version {
    /// Returns true if the editor is installed.
    pub fn is_editor_installed(self) -> anyhow::Result<bool> {
        Ok(Installations::editor_parent_dir()?
            .join(self.to_interned_str())
            .exists())
    }

    /// Returns the path to the editor executable.
    pub fn editor_executable_path(self) -> anyhow::Result<PathBuf> {
        let exe_path = Installations::editor_parent_dir()?
            .join(self.to_interned_str())
            .join(platform::UNITY_EDITOR_EXE);

        if exe_path.exists() {
            Ok(exe_path)
        } else {
            bail!("Unity version is not installed: {self}")
        }
    }
}

#[cfg(test)]
mod editor_dir_tests {
    use super::*;
    use crate::utils::test_util::TempDir;

    #[test]
    fn a_missing_or_empty_directory_has_no_installations() {
        let dir = TempDir::new("editors-empty");
        assert!(!Installations::contains_installations(&dir.path().join("missing")).unwrap());
        assert!(!Installations::contains_installations(dir.path()).unwrap());
    }

    #[test]
    fn a_directory_with_an_editor_has_installations() {
        let dir = TempDir::new("editors-one");
        let exe = dir
            .path()
            .join("6000.0.1f1")
            .join(platform::UNITY_EDITOR_EXE);
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, "").unwrap();
        assert!(Installations::contains_installations(dir.path()).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_directory_is_an_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new("editors-unreadable");
        let editors = dir.path().join("editors");
        fs::create_dir(&editors).unwrap();
        fs::set_permissions(&editors, fs::Permissions::from_mode(0o000)).unwrap();

        // Root can read the directory anyway (for example in a CI container), so there is
        // nothing to test then.
        if fs::read_dir(&editors).is_ok() {
            fs::set_permissions(&editors, fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }

        let result = Installations::contains_installations(&editors);
        fs::set_permissions(&editors, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            result
                .unwrap_err()
                .to_string()
                .starts_with("Cannot read editor directory")
        );
    }
}
