use crate::cli_add::UnityTemplateFile;
use crate::utils::path_ext::PlatformConsistentPathExt;
use anyhow::{Context, anyhow};
use chrono::TimeDelta;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use yansi::{Paint, Style};

pub use crate::commands::add_cmd::add_to_project;
pub use crate::commands::build_cmd::build_project;
pub use crate::commands::info_cmd::project_info;
pub use crate::commands::install_cmd::install_latest_matching;
pub use crate::commands::list_cmd::list_versions;
pub use crate::commands::new_cmd::new_project;
pub use crate::commands::open_cmd::open_project;
pub use crate::commands::run_cmd::run_unity;
pub use crate::commands::updates_cmd::find_project_updates;
use crate::style_definitions::{
    STYLE_ERROR, STYLE_LINK, STYLE_PLAIN, STYLE_UPDATE_AVAILABLE, STYLE_WARNING,
};
use crate::unity::release_api::{UpdatePolicy, fetch_latest_releases};
use crate::unity::release_api_data::{LabelElement, ReleaseIssue};
use crate::unity::{ProjectPath, Version};
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
mod unity_command_builder;

pub const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/Ucom/Editor";
pub const INDENT: &str = "  ";
pub const MARK_UP_TO_DATE: char = '✓';
pub const MARK_UPDATES_AVAILABLE: char = '+';
pub const MARK_UPDATE_TO_LATEST: char = '→';
pub const MARK_NO_INFO: char = '?';
pub const MARK_BULLET: char = '-';
pub const MARK_AVAILABLE: char = '✓';
pub const MARK_UNAVAILABLE: char = '✗';
pub const MARK_ERROR: char = '‼';
pub const MARK_WARNING: char = '!';
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
    fs::write(file_path, content)
        .with_context(|| format!("Failed to write file: {}", file_path.normalized_display()))?;
    Ok(())
}

fn report_error_description(report: &Report, issue: &ReleaseIssue) {
    let Some(label) = issue.label() else {
        return;
    };

    report.blank_line();
    report.header(
        format_args!("{}", label.label_text).paint(issue.style()),
        HeaderLevel::H2,
    );

    let description = report.render_links(&label.description, STYLE_PLAIN, STYLE_LINK);
    let description = report.wrap_text(&description, WrapMode::TerminalWidth);
    report.paragraph(&description);
}

fn format_label_with_url(le: &LabelElement, style: Style) -> String {
    extract_first_url(&le.description).map_or_else(
        || le.label_text.paint(style).to_string(),
        |url| le.label_text.paint(style).link(url).to_string(),
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

/// Checks if the given version has any issues and reports them.
fn report_version_issues(unity_version: Version) {
    let releases = match fetch_latest_releases(UpdatePolicy::Incremental) {
        Ok(releases) => releases,
        Err(e) => {
            eprintln!("Failed to fetch release information: {}", e);
            return;
        }
    };

    let release_data = match releases.get_by_version(unity_version) {
        Ok(release_data) => release_data,
        Err(e) => {
            eprintln!(
                "Failed to get release data for version {}: {}",
                unity_version, e
            );
            return;
        }
    };

    let issue = release_data.issue();
    if issue.has_issue() {
        let report = Report::Terminal;
        report_error_description(&report, &issue);
        report.blank_line();
    }
}

pub fn execute_unity_command(cmd: Command, wait: bool, quiet: bool) -> anyhow::Result<()> {
    if !quiet {
        println!("Running: {}", crate::unity::build_command_line(&cmd));
    }

    if wait {
        crate::unity::wait_with_stdout(cmd)?;
    } else {
        crate::unity::spawn_and_forget(cmd)?;
    }
    Ok(())
}

/// Formats a left-aligned version.
struct AlignedVersion(Version, usize);
impl Display for AlignedVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Needs to be a string otherwise padding won't work
        let v = self.0.to_interned_str();
        let w = self.1;
        write!(f, "{v:<w$}")
    }
}

/// Unified project setup for commands
pub struct ProjectSetup {
    pub project: ProjectPath,
    pub unity_version: Version,
}

impl ProjectSetup {
    /// Initialize project and validate Unity version
    pub fn new(project_dir: &Path) -> anyhow::Result<Self> {
        let project =
            ProjectPath::try_from(project_dir).context("Failed to locate Unity project")?;
        let unity_version = project
            .unity_version()
            .context("Failed to read Unity version from project")?;

        Ok(Self {
            project,
            unity_version,
        })
    }

    /// Get the editor executable for this project's Unity version
    pub fn editor_executable(&self) -> anyhow::Result<PathBuf> {
        self.unity_version.editor_executable_path()
    }
}

#[allow(dead_code)]
impl ReleaseIssue {
    pub fn style(&self) -> Style {
        match self {
            ReleaseIssue::NoIssue => STYLE_PLAIN,
            ReleaseIssue::Error(_) => STYLE_ERROR,
            ReleaseIssue::Warning(_) => STYLE_WARNING,
        }
    }

    pub fn issue_style_or(&self, style: Style) -> Style {
        match self {
            ReleaseIssue::NoIssue => style,
            ReleaseIssue::Error(_) => STYLE_ERROR,
            ReleaseIssue::Warning(_) => STYLE_WARNING,
        }
    }

    pub fn marker(&self) -> String {
        let marker = match self {
            ReleaseIssue::NoIssue => MARK_BULLET,
            ReleaseIssue::Error(_) => MARK_ERROR,
            ReleaseIssue::Warning(_) => MARK_WARNING,
        };

        marker.paint(self.style()).to_string()
    }

    pub fn marker_paint_or<F>(&self, fallback: F, style: Style) -> String
    where
        F: FnOnce() -> char,
    {
        if self.has_issue() {
            self.marker()
        } else {
            fallback().paint(style).to_string()
        }
    }

    pub fn issue_marker_or<F>(&self, fallback: F) -> String
    where
        F: FnOnce() -> String,
    {
        if self.has_issue() {
            self.marker()
        } else {
            fallback()
        }
    }

    pub fn issue_suffix(&self) -> String {
        self.label()
            .map(|label| format!(" [{}]", format_label_with_url(label, self.style())))
            .unwrap_or_default()
    }
}
