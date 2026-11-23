//! The ucom builder script (`UnityBuilder.cs`): where it is installed, its version, and the
//! Unity versions it supports.

use crate::templates::UnityTemplateFile;
use crate::unity::{Major, Minor, ProjectPath, Version};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::warn_non_fatal;
use anyhow::bail;
use std::fs;
use std::path::PathBuf;

/// The project directory that `ucom add builder` and `--inject persistent` install the builder
/// scripts in.
pub const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/Ucom/Editor";

/// The oldest Unity version (major, minor) that the builder scripts (`UnityBuilder.cs` and
/// `EditorMenu.cs`) support. Keep in sync with their `#if UNITY_2021_3_OR_NEWER` guards.
const MIN_BUILDER_VERSION: (Major, Minor) = (2021, 3);

/// The version of the builder script that this ucom comes with: `ScriptVersion` in
/// `templates/UnityBuilder.cs`. Increase both when the editor side gains something an older
/// script cannot do; comment or formatting changes need no new version.
///
/// - 1: scripts without a `ScriptVersion` (everything before version 2).
/// - 2: atomic result files, `wait_for_compilation` and `pre_build_args`.
pub const BUILDER_SCRIPT_VERSION: u32 = 2;

/// Returns an error if the builder scripts do not support the project's Unity version.
pub fn ensure_builder_supports(unity_version: Version) -> anyhow::Result<()> {
    let (major, minor) = MIN_BUILDER_VERSION;
    if (unity_version.major, unity_version.minor) < MIN_BUILDER_VERSION {
        bail!(
            "The ucom build script requires Unity {major}.{minor} or newer; this project uses Unity {unity_version}"
        );
    }
    Ok(())
}

/// Returns the path of the persistent build script, which also contains the editor command watcher.
pub fn persistent_builder_path(project: &ProjectPath) -> PathBuf {
    project
        .join(PERSISTENT_BUILD_SCRIPT_ROOT)
        .join(UnityTemplateFile::Builder.as_asset().filename)
}

/// Returns the version of the builder script installed in the project, or `None` if there is none.
pub fn installed_builder_version(project: &ProjectPath) -> Option<u32> {
    let script = fs::read_to_string(persistent_builder_path(project)).ok()?;
    Some(script_version(&script).unwrap_or(1))
}

/// Reads the `ScriptVersion` constant from the text of a builder script.
fn script_version(script: &str) -> Option<u32> {
    let (_, rest) = script.split_once("const int ScriptVersion = ")?;
    rest.split(';').next()?.trim().parse().ok()
}

/// Warns if the builder script installed in the project is older than the one this ucom comes
/// with, so some features do not work. A changed copy of the current version is not reported.
pub fn warn_if_builder_outdated(project: &ProjectPath) {
    let Some(version) = installed_builder_version(project) else {
        return;
    };
    if version >= BUILDER_SCRIPT_VERSION {
        return;
    }
    warn_non_fatal!(
        "{} in this project is older than this ucom (script version {version}, current \
         {BUILDER_SCRIPT_VERSION}), so newer build features do not work. Update it with:\n\n  \
         ucom add builder --force {}\n\n\
         This replaces the file; apply any changes of your own to the new version again.",
        UnityTemplateFile::Builder.as_asset().filename,
        project.normalized_display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_util::temp_project;

    #[test]
    fn builder_requires_unity_2021_3_or_newer() {
        let supports = |v: &str| ensure_builder_supports(v.parse().unwrap()).is_ok();

        assert!(!supports("2020.3.48f1"));
        assert!(!supports("2021.2.19f1"));
        assert!(supports("2021.3.0f1"));
        assert!(supports("2022.1.0b1"));
        assert!(supports("6000.6.8f1"));
    }

    #[test]
    fn the_embedded_builder_has_the_current_script_version() {
        let script = UnityTemplateFile::Builder.as_asset().content;
        assert_eq!(script_version(script), Some(BUILDER_SCRIPT_VERSION));
    }

    #[test]
    fn the_installed_builder_version_is_read_from_the_script() {
        let (_dir, project) = temp_project("builder-version");
        assert_eq!(installed_builder_version(&project), None);

        let path = persistent_builder_path(&project);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "public const int ScriptVersion = 7;").unwrap();
        assert_eq!(installed_builder_version(&project), Some(7));

        // Scripts from before the version constant.
        fs::write(&path, "public static class UnityBuilder { }").unwrap();
        assert_eq!(installed_builder_version(&project), Some(1));
    }
}
