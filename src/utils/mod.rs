use crate::utils::path_ext::PlatformConsistentPathExt;
use anyhow::{Context, anyhow};
use path_absolutize::Absolutize;
use std::borrow::Cow;
use std::path::Path;

pub mod content_cache;
pub mod path_ext;
pub mod spawn_cmd;
pub mod status_line;
pub mod vec1;

/// Returns the absolute path to an existing directory.
pub fn resolve_absolute_dir_path(path: &impl AsRef<Path>) -> anyhow::Result<Cow<'_, Path>> {
    let path = path.as_ref();

    if cfg!(target_os = "windows") && path.starts_with("~") {
        return Err(anyhow!(
            "On Windows the path cannot start with '~': `{}`",
            path.normalized_display()
        ));
    }

    if !path.is_dir() {
        return Err(anyhow!(
            "Path does not exist or is not a directory: `{}`",
            path.normalized_display()
        ));
    }

    let path = path.absolutize().context("Failed to absolutize the path")?;
    Ok(path)
}
