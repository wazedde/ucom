use std::path::PathBuf;

use clap::{Args, ValueEnum};
use strum::{AsRefStr, Display};

use crate::cli_build::OpenTarget;

#[derive(Args)]
pub struct TestArguments {
    /// The primary mode or platform on which to run tests. Required.
    /// This automatically determines the default build target for the test run
    /// (e.g., 'editmode' uses 'Standalone', 'macos' uses 'OSXUniversal').
    /// Use '--target' to override the default build target.
    #[arg(value_enum, required = true)]
    pub platform: TestTarget,

    /// Path to the Unity project directory containing the tests. Defaults to the current directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Override the active build target for the Unity process running the tests.
    ///
    /// Default is inferred from the selected test platform (e.g., Standalone for editmode/playmode).
    /// Useful for scenarios like running EditMode tests while the project's active target is set to iOS.
    #[arg(short = 't', long, value_name = "NAME")]
    pub target: Option<OpenTarget>,

    /// Control the level of detail for displaying test results in the console.
    #[arg(short = 'r', long, value_name = "RESULTS", default_value = "all")]
    pub show_results: ShowResults,

    /// Run tests with the Unity Editor's graphics device enabled (not in batch mode).
    ///
    /// Disabling batch mode may be necessary for tests requiring graphics
    /// but allows UI popups which can interrupt automated runs.
    #[arg(long)]
    pub no_batch_mode: bool,

    /// Prevent the project path from being added to the Unity Hub/Launcher's history.
    #[arg(long)]
    pub forget_project_path: bool,

    /// Filter tests to run based on categories assigned using the [Category] attribute.
    ///
    /// Provide a semicolon-separated list within quotes (e.g., "Integration;UI").
    /// Use '!' prefix to exclude a category (e.g., "!Slow"). Combines with other filters.
    #[arg(long, value_name = "LIST")]
    pub categories: Option<String>,

    /// Filter tests to run by their full names or name patterns (regex supported).
    ///
    /// Provide a semicolon-separated list within quotes (e.g., "MyNamespace.MyTestClass.MyTestMethod").
    /// Use '!' prefix to exclude a test (e.g., "!FailingTest").
    /// Format for parameterized tests: 'ClassName\.MethodName\(Param1,Param2\)'. Combines with other filters.
    #[arg(long, value_name = "LIST")]
    pub tests: Option<String>,

    /// Filter tests to run based on the assembly they belong to.
    ///
    /// Provide a semicolon-separated list of assembly names within quotes (e.g., "MyTests.dll;AnotherAssembly").
    /// Combines with other filters.
    #[arg(long, value_name = "LIST")]
    pub assemblies: Option<String>,

    /// Suppress informational messages from ucom during the test execution setup.
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show the command that would be used to run Unity tests without actually executing it.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Additional arguments to pass directly to the Unity editor executable running the tests.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

/// Specifies the target environment or platform for running tests.
#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum TestTarget {
    /// Run tests directly within the Unity Editor environment.
    #[value(name = "editmode")]
    EditMode,
    /// Run tests in Play Mode within the Unity Editor environment.
    #[value(name = "playmode")]
    PlayMode,
    /// Run tests in a standalone player build for macOS.
    #[value(name = "macos")]
    StandaloneOSX,
    /// Run tests in a standalone player build for Windows 32-bit.
    #[value(name = "win32")]
    StandaloneWindows,
    /// Run tests in a standalone player build for Windows 64-bit.
    #[value(name = "win64")]
    StandaloneWindows64,
    /// Run tests in a standalone player build for Linux 64-bit.
    #[value(name = "linux64")]
    StandaloneLinux64,
    /// Run tests on an iOS device or simulator (requires additional setup).
    #[value(name = "ios")]
    iOS,
    /// Run tests on an Android device or emulator (requires additional setup).
    #[value(name = "android")]
    Android,
    /// Run tests in a WebGL player build.
    #[value(name = "webgl")]
    WebGL,
}

impl TestTarget {
    pub const fn as_build_target(self) -> OpenTarget {
        match self {
            Self::EditMode | Self::PlayMode => OpenTarget::Standalone,
            Self::StandaloneOSX => OpenTarget::OSXUniversal,
            Self::StandaloneWindows => OpenTarget::Win,
            Self::StandaloneWindows64 => OpenTarget::Win64,
            Self::StandaloneLinux64 => OpenTarget::Linux64,
            Self::iOS => OpenTarget::iOS,
            Self::Android => OpenTarget::Android,
            Self::WebGL => OpenTarget::WebGL,
        }
    }
}

/// Controls the verbosity of test result output.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ShowResults {
    /// Display a full summary of test results, including counts and logs for failures (default).
    #[value(name = "all")]
    All,
    /// Display only information about failed tests.
    #[value(name = "errors")]
    Errors,
    /// Suppress the display of test result summaries in the console output.
    #[value(name = "none")]
    None,
}
