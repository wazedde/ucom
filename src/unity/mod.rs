use std::borrow::Cow;
use std::path::Path;

use anyhow::{anyhow, Context};
use path_absolutize::Absolutize;

pub use crate::unity::project::*;
pub use crate::unity::releases::*;
pub use crate::unity::spawn_cmd::*;
pub use crate::unity::version::*;

pub mod http_cache;
pub mod installed;
pub mod project;
pub mod releases;
pub mod spawn_cmd;
pub mod version;

/// Returns the absolute path to an existing directory.
pub fn to_absolute_dir_path<P: AsRef<Path>>(path: &P) -> anyhow::Result<Cow<'_, Path>> {
    let path = path.as_ref();
    if cfg!(target_os = "windows") && path.starts_with("~") {
        return Err(anyhow!(
            "On Windows the path cannot start with '~': `{}`",
            path.display()
        ));
    }

    if !path.is_dir() {
        return Err(anyhow!(
            "Path does not exist or is not a directory: `{}`",
            path.display()
        ));
    }

    let path = path.absolutize().context("Failed to absolutize the path")?;
    Ok(path)
}
