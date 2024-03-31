use std::fs;
use std::path::Path;

use anyhow::anyhow;
use chrono::TimeDelta;

use crate::cli_add::IncludedFile;
pub(crate) use crate::commands::add_cmd::add_to_project;
pub(crate) use crate::commands::build_cmd::build_project;
pub(crate) use crate::commands::check_cmd::check_updates;
pub(crate) use crate::commands::info_cmd::project_info;
pub(crate) use crate::commands::list_cmd::list_versions;
pub(crate) use crate::commands::new_cmd::new_project;
pub(crate) use crate::commands::open_cmd::open_project;
pub(crate) use crate::commands::run_cmd::run_unity;

mod add_cmd;
mod build_cmd;
mod check_cmd;
mod info_cmd;
mod list_cmd;
mod new_cmd;
mod open_cmd;
mod run_cmd;

pub(crate) mod term_stat;
pub(crate) mod test_cmd;

pub(crate) const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/Ucom/Editor";
pub(crate) const INDENT: &str = "  ";

/// Returns the given time delta as seconds.
pub(crate) fn time_delta_to_seconds(duration: TimeDelta) -> f64 {
    duration.num_seconds() as f64 + duration.subsec_nanos() as f64 * 1e-9
}

/// Adds the given file to the project.
pub(crate) fn add_file_to_project<P: AsRef<Path>, Q: AsRef<Path>>(
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
    fs::write(file_path, content).map_err(Into::into)
}

/// Version of `writeln!` that writes bold text.
macro_rules! writeln_b {
    ($dst:expr $(, $fmt:expr $(, $arg:expr)*)?) => {{
        use std::io::Write;
        let formatted = format!($($fmt $(, $arg)*)?);
        let bold_text = yansi::Paint::new(formatted).bold();
        writeln!($dst, "{}", bold_text)
    }};
}

/// Version of `println!` that writes bold text.
macro_rules! println_b {
    ($($arg:tt)*) => {
        println!("{}", yansi::Paint::new(format!($($arg)*)).bold());
    };
}

/// Version of `println!` that writes bold text if the given condition is true.
macro_rules! println_b_if {
    ($bold:expr, $fmt:expr $(, $arg:expr)*) => {{
        if $bold {
            println!("{}", yansi::Paint::new(format!($fmt $(, $arg)*)).bold());
        } else {
            println!($fmt $(, $arg)*);
        }
    }};
}

pub(crate) use println_b;
pub(crate) use println_b_if;
pub(crate) use writeln_b;
