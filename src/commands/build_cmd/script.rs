//! Injecting the ucom build script (`UnityBuilder.cs`) into a project for a batch build.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use uuid::Uuid;

use crate::cli_add::UnityTemplateFile;
use crate::cli_build::InjectAction;
use crate::commands::builder_script::{PERSISTENT_BUILD_SCRIPT_ROOT, persistent_builder_path};
use crate::commands::project_files::add_file_to_project;
use crate::unity::ProjectPath;
use crate::utils::cancel::CancellableWait;
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::warn_non_fatal;

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
    fn a_persistent_script_is_kept() {
        let (_dir, project) = temp_project("inject");

        let guard = inject_build_script(&project, InjectAction::Persistent).unwrap();

        assert!(guard.is_none());
        assert!(persistent_builder_path(&project).exists());
    }
}
