use std::borrow::Cow;
use std::path::Path;

use anyhow::{anyhow, Context, Error};
use path_absolutize::Absolutize;

pub(crate) use crate::unity::project::*;
pub(crate) use crate::unity::releases::*;
pub(crate) use crate::unity::spawn_cmd::*;
pub(crate) use crate::unity::version::*;

pub(crate) mod http_cache;
pub(crate) mod installed;
pub(crate) mod non_empty_vec;
pub(crate) mod project;
pub(crate) mod releases;
pub(crate) mod spawn_cmd;
pub(crate) mod version;

/// Returns the absolute path to an existing directory.
pub(crate) fn to_absolute_dir_path<P: AsRef<Path>>(path: &P) -> anyhow::Result<Cow<'_, Path>> {
    let path = path.as_ref();
    return inner(path);

    fn inner(path: &Path) -> Result<Cow<'_, Path>, Error> {
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
}
