//! Injecting the ucom build script (`UnityBuilder.cs`) into a project for a batch build.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use uuid::Uuid;

use crate::cli_add::UnityTemplateFile;
use crate::cli_build::InjectAction;
use crate::commands::{PERSISTENT_BUILD_SCRIPT_ROOT, add_file_to_project, warn_non_fatal};
use crate::unity::ProjectPath;
use crate::utils::cancel::CancellableWait;
use crate::utils::path_ext::PlatformConsistentPathExt;

const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/Ucom";

/// A build script injected for one build (`--inject auto`). Removes it from the project when
/// dropped, which also covers early returns, panics and Ctrl+C.
#[must_use = "the injected script is removed when this guard is dropped"]
pub(super) struct InjectedScript {
    /// The unique directory in the project that holds the script.
    dir: PathBuf,
    /// While the script is in the project, Ctrl+C cancels instead of ending ucom, so that this
    /// guard is dropped.
    _cancellable: CancellableWait,
}

impl Drop for InjectedScript {
    fn drop(&mut self) {
        if let Err(e) = cleanup_csharp_build_script(&self.dir) {
            warn_non_fatal!("{e:#}");
        }
    }
}

/// Returns the path of the persistent build script, which also contains the editor command watcher.
pub(super) fn persistent_builder_path(project: &ProjectPath) -> PathBuf {
    project
        .join(PERSISTENT_BUILD_SCRIPT_ROOT)
        .join(UnityTemplateFile::Builder.as_asset().filename)
}

/// The version of the builder script that this ucom comes with: `ScriptVersion` in
/// `templates/UnityBuilder.cs`. Increase both when the editor side gains something an older
/// script cannot do; comment or formatting changes need no new version.
///
/// - 1: scripts without a `ScriptVersion` (everything before version 2).
/// - 2: atomic result files, `wait_for_compilation` and `pre_build_args`.
pub(super) const BUILDER_SCRIPT_VERSION: u32 = 2;

/// Returns the version of the builder script installed in the project, or `None` if there is none.
pub(super) fn installed_builder_version(project: &ProjectPath) -> Option<u32> {
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
pub(super) fn warn_if_builder_outdated(project: &ProjectPath) {
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

/// Injects the build script into the project, unless it is already there or `inject` is `Off`.
///
/// For `Auto`, returns a guard that removes the script again when dropped. `Persistent` leaves
/// the script in the project.
#[must_use = "dropping the result removes an injected script at once"]
pub(super) fn inject_build_script(
    project: &ProjectPath,
    inject: InjectAction,
) -> anyhow::Result<Option<InjectedScript>> {
    match inject {
        // Build script is already present, no need to inject.
        InjectAction::Auto | InjectAction::Persistent
            if persistent_builder_path(project).exists() =>
        {
            Ok(None)
        }

        // Build script is not present, inject it in a unique directory to avoid conflicts.
        InjectAction::Auto => {
            let unique_dir_name = format!("{AUTO_BUILD_SCRIPT_ROOT}-{}", Uuid::new_v4());
            // Created before the script is written, so a partly written script is removed too.
            let guard = InjectedScript {
                dir: project.join(&unique_dir_name),
                _cancellable: CancellableWait::start(),
            };
            add_file_to_project(
                project,
                PathBuf::from(&unique_dir_name).join("Editor"),
                UnityTemplateFile::Builder,
            )?;
            Ok(Some(guard))
        }

        // Build script is not present, inject it.
        InjectAction::Persistent => {
            add_file_to_project(
                project,
                PERSISTENT_BUILD_SCRIPT_ROOT,
                UnityTemplateFile::Builder,
            )?;
            Ok(None)
        }

        // No need to do anything.
        InjectAction::Off => Ok(None),
    }
}

/// Removes the injected build script from the project.
fn cleanup_csharp_build_script(parent_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let parent_dir = parent_dir.as_ref();
    if !parent_dir.exists() {
        return Ok(());
    }

    // Remove the directory where the build script is located.
    fs::remove_dir_all(parent_dir).with_context(|| {
        format!(
            "Could not remove temporary directory: {}",
            parent_dir.normalized_display()
        )
    })?;

    // Remove the .meta file.
    let meta_file = parent_dir.with_extension("meta");
    if meta_file.exists() {
        fs::remove_file(&meta_file)
            .with_context(|| format!("Could not remove: {}", meta_file.normalized_display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_add::AssetSource;
    use crate::utils::test_util::temp_project;

    fn auto_script_dirs(project: &ProjectPath) -> Vec<PathBuf> {
        let assets = project.join("Assets");
        if !assets.exists() {
            return Vec::new();
        }
        fs::read_dir(assets)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("Ucom-")
            })
            .collect()
    }

    #[test]
    fn an_auto_injected_script_is_removed_when_the_guard_drops() {
        let (_dir, project) = temp_project("inject");

        let guard = inject_build_script(&project, InjectAction::Auto).unwrap();
        let dirs = auto_script_dirs(&project);
        assert_eq!(dirs.len(), 1);
        assert!(dirs[0].join("Editor/UnityBuilder.cs").exists());
        // Unity creates a .meta file next to the directory.
        fs::write(dirs[0].with_extension("meta"), "").unwrap();

        drop(guard);

        assert!(auto_script_dirs(&project).is_empty());
        assert!(!dirs[0].with_extension("meta").exists());
    }

    #[test]
    fn an_auto_injected_script_is_removed_on_an_early_return() {
        let (_dir, project) = temp_project("inject");

        let build = || -> anyhow::Result<()> {
            let _guard = inject_build_script(&project, InjectAction::Auto)?;
            anyhow::bail!("build failed")
        };

        assert!(build().is_err());
        assert!(auto_script_dirs(&project).is_empty());
    }

    #[test]
    fn the_embedded_builder_has_the_current_script_version() {
        let AssetSource::Static(script) = UnityTemplateFile::Builder.as_asset().content else {
            unreachable!("the builder script is embedded")
        };
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

    #[test]
    fn a_persistent_script_is_kept() {
        let (_dir, project) = temp_project("inject");

        let guard = inject_build_script(&project, InjectAction::Persistent).unwrap();

        assert!(guard.is_none());
        assert!(persistent_builder_path(&project).exists());
    }
}
