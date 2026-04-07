use std::path::PathBuf;
use std::process::Command;

/// Builder pattern for constructing Unity editor commands.
///
/// This abstraction consolidates the repeated pattern of building Unity command-line invocations
/// across multiple commands (build, test, open, run, new).
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
