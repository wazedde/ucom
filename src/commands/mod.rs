use crate::cli::IncludedFile;
pub use crate::commands::build_project::build_project;
pub use crate::commands::check_updates::check_updates;
pub use crate::commands::list_versions::list_versions;
pub use crate::commands::new_project::new_project;
pub use crate::commands::open_project::open_project;
pub use crate::commands::project_info::project_info;
pub use crate::commands::run_unity::run_unity;
use anyhow::anyhow;
use colored::{ColoredString, Colorize};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub mod build_project;
pub mod check_updates;
pub mod list_versions;
pub mod new_project;
pub mod open_project;
pub mod project_info;
pub mod run_unity;
pub mod terminal_spinner;

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

// Adds the given file to the project.
fn add_file_to_project(file_path: &Path, content: &str) -> anyhow::Result<()> {
    let parent_dir = file_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid file path: {}", file_path.display()))?;

    fs::create_dir_all(parent_dir)?;

    let mut file = File::create(file_path)?;
    write!(file, "{}", content).map_err(Into::into)
}

/// Adds the given file to the project.
pub fn add_template_to_project(
    project_path: PathBuf,
    file_dir: PathBuf,
    template: IncludedFile,
) -> anyhow::Result<()> {
    let data = template.data();
    let file_path = file_dir.join(data.filename);
    println!("Adding file to project: {}", file_path.display());
    add_file_to_project(&project_path.join(file_path), data.content)
}
