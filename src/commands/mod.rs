use std::fs;
use std::path::Path;

use anyhow::anyhow;
use chrono::TimeDelta;

use crate::cli_add::UnityTemplateFile;
pub use crate::commands::add_cmd::add_to_project;
pub use crate::commands::build_cmd::build_project;
pub use crate::commands::info_cmd::project_info;
pub use crate::commands::install_cmd::install_latest_matching;
pub use crate::commands::list_cmd::list_versions;
pub use crate::commands::new_cmd::new_project;
pub use crate::commands::open_cmd::open_project;
pub use crate::commands::run_cmd::run_unity;
pub use crate::commands::updates_cmd::find_project_updates;

mod add_cmd;
mod build_cmd;
mod info_cmd;
mod install_cmd;
mod list_cmd;
mod new_cmd;
mod open_cmd;
mod run_cmd;
mod updates_cmd;

pub mod test_cmd;

pub const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/Ucom/Editor";
pub const INDENT: &str = "  ";

trait TimeDeltaExt {
    fn as_seconds(&self) -> f64;
}

impl TimeDeltaExt for TimeDelta {
    /// Returns the given time delta as seconds.
    fn as_seconds(&self) -> f64 {
        (*self).num_milliseconds() as f64 / 1000f64
    }
}

/// Adds the given file to the project.
fn add_file_to_project(
    project_root: impl AsRef<Path>,
    destination_dir: impl AsRef<Path>,
    template_file: UnityTemplateFile,
) -> anyhow::Result<()> {
    let template_data = template_file.as_asset();
    let file_path = destination_dir.as_ref().join(template_data.filename);
    let content = template_data.load_content()?;

    create_file(project_root.as_ref().join(&file_path), &content)
        .inspect(|()| println!("Added to project: {}", file_path.display()))
        .inspect_err(|_| println!("Failed to add file to project: {}", file_path.display()))
}

fn create_file(file_path: impl AsRef<Path>, content: &str) -> anyhow::Result<()> {
    let file_path = file_path.as_ref();
    let parent_dir = file_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid file path: {}", file_path.display()))?;

    fs::create_dir_all(parent_dir)?;
    fs::write(file_path, content).map_err(Into::into)
}

/// Version of `println!` that writes bold text.
macro_rules! println_bold {
    ($($arg:tt)*) => {
        println!("{}", yansi::Paint::new(format!($($arg)*)).bold());
    };
}

/// Version of `println!` that writes bold text if the given condition is true.
macro_rules! println_conditional_bold {
    ($bold:expr, $fmt:expr $(, $arg:expr)*) => {{
        if $bold {
            println!("{}", yansi::Paint::new(format!($fmt $(, $arg)*)).bold());
        } else {
            println!($fmt $(, $arg)*);
        }
    }};
}

use println_bold;
use println_conditional_bold;
