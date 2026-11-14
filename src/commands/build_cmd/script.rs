//! Injecting the ucom build script (`UnityBuilder.cs`) into a project for a batch build.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use uuid::Uuid;

use crate::cli_add::UnityTemplateFile;
use crate::cli_build::InjectAction;
use crate::commands::{PERSISTENT_BUILD_SCRIPT_ROOT, add_file_to_project};
use crate::unity::ProjectPath;
use crate::utils::path_ext::PlatformConsistentPathExt;

const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/Ucom";

/// Represents a hook function that returns a result.
type HookFn = Box<dyn FnOnce() -> anyhow::Result<()>>;

/// Returns a no-op hook function.
fn no_op_hook() -> HookFn {
    Box::new(|| Ok(()))
}

/// Represents the build hooks for injecting and cleaning up the build script.
pub(super) struct BuildHooks {
    pub(super) inject_build_script: HookFn,
    pub(super) cleanup_build_script: HookFn,
}

impl BuildHooks {
    /// Creates a new `BuildHooks` instance with the specified hooks.
    fn new(inject_build_script: HookFn, cleanup_build_script: HookFn) -> Self {
        Self {
            inject_build_script,
            cleanup_build_script,
        }
    }

    /// Returns a no-op `BuildHooks` instance.
    fn no_op() -> Self {
        Self::new(no_op_hook(), no_op_hook())
    }
}

/// Returns the path of the persistent build script, which also contains the editor command watcher.
pub(super) fn persistent_builder_path(project: &ProjectPath) -> PathBuf {
    project
        .join(PERSISTENT_BUILD_SCRIPT_ROOT)
        .join(UnityTemplateFile::Builder.as_asset().filename)
}

/// Creates actions that inject a script into the project before and after the build.
pub(super) fn csharp_build_script_injection_hooks(
    project: &ProjectPath,
    inject: InjectAction,
) -> BuildHooks {
    let persistent_script_exists = persistent_builder_path(project).exists();

    match inject {
        // Build script is already present, no need to inject.
        InjectAction::Auto | InjectAction::Persistent if persistent_script_exists => {
            BuildHooks::no_op()
        }

        // Build script is not present, inject it in a unique directory to avoid conflicts.
        InjectAction::Auto => {
            let uuid = Uuid::new_v4();
            let unique_dir_name = format!("{AUTO_BUILD_SCRIPT_ROOT}-{uuid}");
            let closure_project_dir = project.to_path_buf();
            let closure_script_dir = PathBuf::from(&unique_dir_name).join("Editor");
            let closure_remove_dir = project.join(&unique_dir_name);

            BuildHooks::new(
                Box::new(|| {
                    add_file_to_project(
                        closure_project_dir,
                        closure_script_dir,
                        UnityTemplateFile::Builder,
                    )
                }),
                Box::new(|| cleanup_csharp_build_script(closure_remove_dir)),
            )
        }

        // Build script is not present, inject it.
        InjectAction::Persistent => {
            let closure_project_dir = project.to_path_buf();
            let closure_script_dir = PathBuf::from(PERSISTENT_BUILD_SCRIPT_ROOT);

            BuildHooks::new(
                Box::new(|| {
                    add_file_to_project(
                        closure_project_dir,
                        closure_script_dir,
                        UnityTemplateFile::Builder,
                    )
                }),
                no_op_hook(),
            )
        }

        // No need to do anything.
        InjectAction::Off => BuildHooks::no_op(),
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
