use std::fs;
use std::path::Path;

use crate::cli_add::UnityTemplateFile;
use crate::utils::path_ext::PlatformConsistentPathExt;
use anyhow::anyhow;
use chrono::TimeDelta;
use yansi::Paint;

pub use crate::commands::add_cmd::add_to_project;
pub use crate::commands::build_cmd::build_project;
pub use crate::commands::info_cmd::project_info;
pub use crate::commands::install_cmd::install_latest_matching;
pub use crate::commands::list_cmd::list_versions;
pub use crate::commands::new_cmd::new_project;
pub use crate::commands::open_cmd::open_project;
pub use crate::commands::run_cmd::run_unity;
pub use crate::commands::updates_cmd::find_project_updates;
use crate::style_definitions::{ERROR, LINK, UNSTYLED};
use crate::unity::release_api_data::LabelElement;
use crate::utils::report::{HeaderLevel, Report, WrapMode};

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
pub const MARK_UP_TO_DATE: char = '✓';
pub const MARK_UPDATES_AVAILABLE: char = '!';
pub const MARK_UPDATE_TO_LATEST: char = '>';
pub const MARK_NO_INFO: char = '?';
pub const MARK_BULLET: char = '-';
pub const MARK_AVAILABLE: char = '✓';
pub const MARK_UNAVAILABLE: char = '✗';
pub const MARK_ERROR: char = '‼';
pub const MARK_SUGGESTED: char = '*';

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
        .inspect(|()| {
            println!(
                "{INDENT}Added to project: {}",
                file_path.normalized_display()
            );
        })
        .inspect_err(|_| {
            println!(
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
    fs::write(file_path, content).map_err(Into::into)
}

fn report_error_description(report: &Report, error_label: &LabelElement) {
    report.blank_line();
    report.header(
        format_args!("{}", error_label.label_text).paint(ERROR),
        HeaderLevel::H2,
    );

    let description = report.render_links(&error_label.description, UNSTYLED, LINK);
    let description = report.wrap_text(&description, WrapMode::TerminalWidth);
    report.paragraph(&description);
}

fn format_label_with_url(le: &LabelElement) -> String {
    extract_first_url(&le.description).map_or_else(
        || le.label_text.paint(ERROR).to_string(),
        |url| le.label_text.paint(ERROR).link(url).to_string(),
    )
}

fn extract_first_url(text: &str) -> Option<&str> {
    let start_index = text.find("http://").or_else(|| text.find("https://"))?;
    let remaining_text = &text[start_index..];
    let end_index = remaining_text
        .find(|c: char| c.is_whitespace() || c == '"' || c == '<' || c == '>')
        .unwrap_or(remaining_text.len());

    Some(&remaining_text[..end_index])
}
