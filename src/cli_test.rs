use std::path::PathBuf;

use clap::{Args, ValueEnum};
use strum::{AsRefStr, Display};

use crate::cli_build::OpenTarget;

#[derive(Args)]
pub struct TestArguments {
    /// Platform to run tests on
    ///
    /// Build target is automatically determined by platform.
    /// 'editmode' and 'playmode' use 'standalone' build target
    /// 'macos' uses 'macos' build target, etc.
    /// Use --target to override this behavior.
    #[arg(value_enum)]
    pub platform: TestTarget,

    /// Project directory path
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Set active build target
    ///
    /// Default build target matches test platform.
    /// Override to run tests with different build target
    /// (e.g., editmode tests with ios target)
    #[arg(short = 't', long, value_name = "NAME")]
    pub target: Option<OpenTarget>,

    /// Test results display level
    #[arg(short = 'r', long, value_name = "RESULTS", default_value = "all")]
    pub show_results: ShowResults,

    /// Disable batch mode
    ///
    /// Batch mode prevents manual inputs but disables graphics device
    /// which may cause some tests to fail
    #[arg(long)]
    pub no_batch_mode: bool,

    /// Skip adding project to Unity launcher/hub history
    #[arg(long)]
    pub forget_project_path: bool,

    /// Filter by test categories
    ///
    /// Use semicolon-separated list in quotes: "category1;category2"
    /// Works with --tests to run only tests matching both filters
    /// Use negation with ! prefix: "!excludedCategory"
    #[arg(long, value_name = "LIST")]
    pub categories: Option<String>,

    /// Filter by test names or regex pattern
    ///
    /// Use semicolon-separated list in quotes: "Test1;Test2"
    /// Supports negation with ! prefix: "!TestToExclude"
    /// For parameterized tests: "ClassName\.MethodName\(Param1,Param2\)"
    #[arg(long, value_name = "LIST")]
    pub tests: Option<String>,

    /// Filter by test assemblies
    ///
    /// Use semicolon-separated list in quotes: "Assembly1;Assembly2"
    #[arg(long, value_name = "LIST")]
    pub assemblies: Option<String>,

    /// Suppress messages
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show command without executing
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Arguments to pass directly to Unity
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

/// Build target for testing
#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum TestTarget {
    #[value(name = "editmode")]
    EditMode,
    #[value(name = "playmode")]
    PlayMode,
    #[value(name = "macos")]
    StandaloneOSX,
    #[value(name = "win32")]
    StandaloneWindows,
    #[value(name = "win64")]
    StandaloneWindows64,
    #[value(name = "linux64")]
    StandaloneLinux64,
    #[value(name = "ios")]
    iOS,
    #[value(name = "android")]
    Android,
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

/// Result display level
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ShowResults {
    /// Display all results
    #[value(name = "all")]
    All,
    /// Display only errors
    #[value(name = "errors")]
    Errors,
    /// Display no results
    #[value(name = "none")]
    None,
}
