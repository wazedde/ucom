use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use clap::{Args, ValueEnum};

use crate::cli_build::OpenTarget;

#[derive(Args)]
pub struct TestArguments {
    /// The platform to run tests on.
    ///
    /// The build target to open the project with is automatically determined by the platform.
    /// E.g., `editmode` and `playmode' will open the project with the `standalone` build target
    /// and `macos' will open the project with the `macos` build target.
    /// If you want to override this, you can use the `--target` option.
    #[arg(value_enum)]
    pub platform: TestTarget,

    /// Specifies the project's directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Determines the active build target to open the project with.
    ///
    /// By default, the build target matches the specified test platform.
    /// However, you can override this by specifying a different build target. For example to run
    /// `editmode` tests using the `ios` build target.
    #[arg(short = 't', long, value_name = "NAME")]
    pub target: Option<OpenTarget>,

    /// The type of test results to display.
    #[arg(short = 'r', long, value_name = "RESULTS", default_value = "all")]
    pub show_results: ShowResults,

    /// Suppresses running Unity in batch mode.
    ///
    /// Running tests in batch mode removes the need for manual user inputs, but it also disables
    /// the graphics device and may cause some tests to fail.
    #[arg(long)]
    pub no_batch_mode: bool,

    /// Don't save your current Project into the Unity launcher/hub history.
    #[arg(long)]
    pub forget_project_path: bool,

    /// A semicolon-separated list of test categories to include in the run.
    ///
    /// A semi-colon separated list should be formatted as a string enclosed in quotation marks,
    /// e.g. `categories "firstCategory;secondCategory"`.
    /// If using both `categories` and `tests`, then only test that matches both are run.
    /// This argument supports negation using '!'.
    /// If using '!MyCategory' then no tests with the 'MyCategory' category will be included in the run.
    #[arg(long, value_name = "LIST")]
    pub categories: Option<String>,

    /// A semicolon-separated list of test names to run,
    /// or a regular expression pattern to match tests by their full name.
    ///
    /// A semi-colon separated list should be formatted as a string enclosed in quotation marks,
    /// e.g. `tests "Low;Medium"`.
    /// This argument supports negation using '!'.
    /// If using the test filter '!MyNamespace.Something.MyTest',
    /// then all tests except that test will be run.
    /// It is also possible to run a specific variation of a parameterized test like so:
    /// `"ClassName\.MethodName\(Param1,Param2\)"`
    #[arg(long, value_name = "LIST")]
    pub tests: Option<String>,

    /// A semicolon-separated list of test assemblies to include in the run.
    ///
    /// A semi-colon separated list should be formatted as a string enclosed in quotation marks,
    /// e.g. `assemblyNames "firstAssembly;secondAssembly"`.
    #[arg(long, value_name = "LIST")]
    pub assemblies: Option<String>,

    /// Suppresses ucom messages.
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Shows the command to be run without actually executing it.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments to be passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

/// The build target to open the project with.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
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
    pub fn as_build_target(self) -> OpenTarget {
        match self {
            TestTarget::EditMode | TestTarget::PlayMode => OpenTarget::Standalone,
            TestTarget::StandaloneOSX => OpenTarget::OSXUniversal,
            TestTarget::StandaloneWindows => OpenTarget::Win,
            TestTarget::StandaloneWindows64 => OpenTarget::Win64,
            TestTarget::StandaloneLinux64 => OpenTarget::Linux64,
            TestTarget::iOS => OpenTarget::iOS,
            TestTarget::Android => OpenTarget::Android,
            TestTarget::WebGL => OpenTarget::WebGL,
        }
    }
}

impl Display for TestTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The type of test results to display.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ShowResults {
    /// Display all results.
    #[value(name = "all")]
    All,
    /// Only display errors.
    #[value(name = "errors")]
    Errors,
    /// Don't display any results.
    #[value(name = "none")]
    None,
}
