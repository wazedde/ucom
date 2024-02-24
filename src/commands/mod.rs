use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::cli_add::IncludedFile;
use anyhow::anyhow;
use chrono::TimeDelta;
use colored::{ColoredString, Colorize};

pub use crate::commands::add_cmd::add_to_project;
pub use crate::commands::build_cmd::build_project;
pub use crate::commands::check_cmd::check_updates;
pub use crate::commands::info_cmd::project_info;
pub use crate::commands::list_cmd::list_versions;
pub use crate::commands::new_cmd::new_project;
pub use crate::commands::open_cmd::open_project;
pub use crate::commands::run_cmd::run_unity;

mod add_cmd;
mod build_cmd;
mod check_cmd;
mod info_cmd;
mod list_cmd;
mod new_cmd;
mod open_cmd;
mod run_cmd;

pub mod term_stat;
pub mod test_cmd;

pub const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/Ucom/Editor";
pub const INDENT: &str = "  ";

pub trait ColoredStringIf {
    /// Returns bold string if `is_bold` is true.
    fn bold_if(&self, is_bold: bool) -> ColoredString;
}

impl ColoredStringIf for str {
    fn bold_if(&self, is_bold: bool) -> ColoredString {
        if is_bold {
            self.bold()
        } else {
            self.into()
        }
    }
}

/// Returns the given time delta as seconds.
pub fn time_delta_to_seconds(duration: TimeDelta) -> f64 {
    duration.num_seconds() as f64 + duration.subsec_nanos() as f64 * 1e-9
}

/// Adds the given file to the project.
pub fn add_file_to_project<P: AsRef<Path>, Q: AsRef<Path>>(
    project_dir: P,
    file_dir: Q,
    template: IncludedFile,
) -> anyhow::Result<()> {
    let data = template.data();
    let file_path = file_dir.as_ref().join(data.filename);
    let content = data.fetch_content()?;
    println!("Added to project: {}", file_path.display());
    create_file(project_dir.as_ref().join(file_path), &content)
}

fn create_file<P: AsRef<Path>>(file_path: P, content: &str) -> anyhow::Result<()> {
    let file_path = file_path.as_ref();
    let parent_dir = file_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid file path: {}", file_path.display()))?;

    fs::create_dir_all(parent_dir)?;

    let mut file = File::create(file_path)?;
    write!(file, "{}", content).map_err(Into::into)
}
