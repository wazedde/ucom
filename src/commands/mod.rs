use crate::cli_add::UnityTemplateFile;
use crate::utils::path_ext::PlatformConsistentPathExt;
use anyhow::{Context, anyhow};
use chrono::TimeDelta;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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
use crate::unity::release_api::{UpdatePolicy, fetch_latest_releases};
use crate::unity::release_api_data::LabelElement;
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

/// Checks if the given version has any issues and reports them.
fn check_version_issues(unity_version: Version) {
    let releases = match fetch_latest_releases(UpdatePolicy::Incremental) {
        Ok(releases) => releases,
        Err(e) => {
            eprintln!("Failed to fetch release information: {}", e);
            return;
        }
    };

    let release_data = match releases.get_by_version(unity_version) {
        Ok(data) => data,
        Err(e) => {
            eprintln!(
                "Failed to get release data for version {}: {}",
                unity_version, e
            );
            return;
        }
    };

    release_data.error_label().inspect(|label| {
        let report = Report::Terminal;
        report_error_description(&report, label);
        report.blank_line();
    });
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

/// Builder pattern for constructing Unity editor commands.
///
/// This abstraction consolidates the repeated pattern of building Unity command-line invocations
/// across multiple commands (build, test, open, run, new). It provides a fluent API for setting
/// common Unity flags and arguments.
///
/// # Example
///
/// ```ignore
/// let cmd = UnityCommandBuilder::new(editor_path, project_path)
///     .with_build_target("WebGL")
///     .batch_mode(true)
///     .quit(true)
///     .build();
/// ```
pub struct UnityCommandBuilder {
    editor_path: PathBuf,
    project_path: Option<PathBuf>,
    build_target: Option<String>,
    batch_mode: bool,
    no_graphics: bool,
    quit: bool,
    log_file: Option<PathBuf>,
    additional_args: Vec<String>,
}

impl UnityCommandBuilder {
    /// Create a new Unity command builder with the editor executable path.
    ///
    /// For commands that don't operate on a project (like `run`), you can omit the project path
    /// and add arguments directly.
    pub fn new(editor_path: PathBuf) -> Self {
        Self {
            editor_path,
            project_path: None,
            build_target: None,
            batch_mode: false,
            no_graphics: false,
            quit: false,
            log_file: None,
            additional_args: Vec::new(),
        }
    }

    /// Set the project path. This will add `-projectPath <path>` to the command.
    pub fn with_project_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.project_path = Some(path.into());
        self
    }

    /// Set the build target. This will add `-buildTarget <target>` to the command.
    pub fn with_build_target(mut self, target: impl AsRef<str>) -> Self {
        self.build_target = Some(target.as_ref().to_string());
        self
    }

    /// Enable batch mode. This will add `-batchmode` to the command.
    pub fn batch_mode(mut self, enabled: bool) -> Self {
        self.batch_mode = enabled;
        self
    }

    /// Enable no-graphics mode. This will add `-nographics` to the command.
    pub fn no_graphics(mut self, enabled: bool) -> Self {
        self.no_graphics = enabled;
        self
    }

    /// Enable quit flag. This will add `-quit` to the command.
    pub fn quit(mut self, enabled: bool) -> Self {
        self.quit = enabled;
        self
    }

    /// Set the log file path. This will add `-logFile <path>` to the command.
    pub fn with_log_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.log_file = Some(path.into());
        self
    }

    /// Add a single argument to the command.
    pub fn add_arg(mut self, arg: impl Into<String>) -> Self {
        self.additional_args.push(arg.into());
        self
    }

    /// Add multiple arguments to the command.
    pub fn add_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.additional_args
            .extend(args.into_iter().map(Into::into));
        self
    }

    /// Build the final Command with all configured options.
    pub fn build(self) -> Command {
        let mut cmd = Command::new(&self.editor_path);

        if let Some(project_path) = self.project_path {
            cmd.args(["-projectPath", &project_path.to_string_lossy()]);
        }

        if let Some(target) = self.build_target {
            cmd.args(["-buildTarget", &target]);
        }

        if let Some(log_file) = self.log_file {
            cmd.args(["-logFile", &log_file.to_string_lossy()]);
        }

        if self.batch_mode {
            cmd.arg("-batchmode");
        }

        if self.no_graphics {
            cmd.arg("-nographics");
        }

        if self.quit {
            cmd.arg("-quit");
        }

        for arg in self.additional_args {
            cmd.arg(arg);
        }

        cmd
    }
}
