//! Adding ucom's template files to a Unity project.

use crate::cli_add::UnityTemplateFile;
use crate::commands::output::INDENT;
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::warn_non_fatal;
use anyhow::{Context, anyhow};
use std::fs;
use std::path::Path;

/// Adds the given file to the project.
pub fn add_file_to_project(
    project_root: impl AsRef<Path>,
    destination_dir: impl AsRef<Path>,
    template_file: UnityTemplateFile,
) -> anyhow::Result<()> {
    let template_data = template_file.as_asset();
    let file_path = destination_dir.as_ref().join(template_data.filename);
    let content = template_data.load_content()?;

    create_file(project_root.as_ref().join(&file_path), &content)
        .inspect(|()| {
            println!(
                "{INDENT}Added to project: {}",
                file_path.normalized_display()
            );
        })
        .inspect_err(|_| {
            warn_non_fatal!(
                "{INDENT}Failed to add file to project: {}",
                file_path.normalized_display()
            );
        })
}

fn create_file(file_path: impl AsRef<Path>, content: &str) -> anyhow::Result<()> {
    let file_path = file_path.as_ref();
    let parent_dir = file_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid file path: {}", file_path.normalized_display()))?;

    fs::create_dir_all(parent_dir)?;
    fs::write(file_path, content)
        .with_context(|| format!("Failed to write file: {}", file_path.normalized_display()))?;
    Ok(())
}
