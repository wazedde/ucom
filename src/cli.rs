use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use clap::{Args, ValueEnum};

pub const ENV_EDITOR_DIR: &str = "UCOM_EDITOR_DIR";
pub const ENV_DEFAULT_VERSION: &str = "UCOM_DEFAULT_VERSION";
pub const ENV_BUILD_TARGET: &str = "UCOM_BUILD_TARGET";
pub const ENV_PACKAGE_LEVEL: &str = "UCOM_PACKAGE_LEVEL";

/// Unity Commander: A command-line interface for Unity projects.
#[derive(clap::Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Disables colored output.
    #[arg(long, short = 'D')]
    pub disable_color: bool,

    #[command(subcommand)]
    pub command: Option<Action>,
}

#[derive(clap::Subcommand)]
pub enum Action {
    /// Lists installed Unity versions.
    #[command(visible_alias = "l")]
    List {
        /// Defines what to list.
        #[arg(value_enum, default_value = "installed")]
        list_type: ListType,

        /// Filters the Unity versions to list based on the pattern. For example, '2021' will list all 2021.x.y versions.
        #[arg(short = 'u', long = "unity", value_name = "VERSION")]
        version_pattern: Option<String>,
    },

    /// Displays project information.
    #[command(visible_alias = "i")]
    Info {
        /// Specifies the project's directory.
        #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
        project_dir: PathBuf,

        /// Determines the level of package information to display.
        #[arg(short='p', long, default_value = "no-unity", env = ENV_PACKAGE_LEVEL)]
        packages: PackagesInfoLevel,
    },

    /// Checks the Unity website for updates to the project's version.
    #[command(visible_alias = "c")]
    Check {
        /// Specifies the project's directory.
        #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
        project_dir: PathBuf,

        /// Generates a Markdown report of aggregated release notes.
        #[clap(short = 'r', long)]
        report: bool,
    },

    /// Creates a new Unity project and Git repository, defaulting to the latest installed Unity version.
    #[command(visible_alias = "n")]
    New(NewArguments),

    /// Opens a specified Unity project in the Unity Editor.
    #[command(visible_alias = "o")]
    Open(OpenArguments),

    /// Builds a specified Unity project.
    #[command(visible_alias = "b")]
    Build(BuildArguments),

    /// Runs Unity with specified arguments, defaulting to the latest installed Unity version.
    #[command(visible_alias = "r")]
    Run(RunArguments),

    /// Prints the specified template to standard output.
    #[command()]
    Template {
        #[arg(value_enum)]
        template: Template,
    },
    /// Purges the download cache.
    ///
    /// Unity release data, once downloaded, is stored for an hour to improve performance.
    /// Set the `UCOM_ENABLE_CACHE` environment variable to `false` to turn off the download cache.
    #[command()]
    ClearCache,
}

#[derive(Args)]
pub struct RunArguments {
    /// Specifies the Unity version to run. For example, '2021' runs the latest installed 2021.x.y version.
    #[arg(
        short = 'u',
        long = "unity",
        value_name = "VERSION",
        env = ENV_DEFAULT_VERSION,
    )]
    pub version_pattern: Option<String>,

    /// Waits for the command to complete before proceeding.
    #[clap(short = 'w', long)]
    pub wait: bool,

    /// Suppresses ucom messages.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Displays the command to be run without actually executing it.
    #[clap(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments to be passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS", required = true)]
    pub args: Option<Vec<String>>,
}

#[derive(Args)]
pub struct NewArguments {
    /// Specifies the Unity version for the new project. For example, '2021' uses the latest installed 2021.x.y version.
    #[arg(
        short = 'u',
        long = "unity",
        value_name = "VERSION",
        env = ENV_DEFAULT_VERSION
    )]
    pub version_pattern: Option<String>,

    /// Defines the directory for creating the project. This directory should not pre-exist.
    #[arg(
        required = true,
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::DirPath
    )]
    pub project_dir: PathBuf,

    /// Initializes LFS for the repository and includes a .gitattributes file with Unity-specific LFS settings.
    #[clap(long = "lfs")]
    pub include_lfs: bool,

    /// Skips initialization of a new Git repository.
    #[clap(long)]
    pub no_git: bool,

    /// Waits for the command to complete before proceeding.
    #[clap(short = 'w', long)]
    pub wait: bool,

    /// Closes the editor after the project creation.
    #[clap(short = 'Q', long)]
    pub quit: bool,

    /// Suppresses ucom messages.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Shows the command to be run without actually executing it.
    #[clap(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments to be passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

#[derive(Args)]
pub struct OpenArguments {
    /// Specifies the project's directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Upgrades the project's Unity version.
    /// A partial version like '2021' selects the latest installed version within the 2021.x.y range.
    /// If no version is specified, it defaults to the latest available version within the project's major.minor range.
    #[arg(short = 'U', long = "upgrade", value_name = "VERSION")]
    pub upgrade_version: Option<Option<String>>,

    /// Determines the active build target to open the project with.
    #[arg(short = 't', long, value_name = "NAME")]
    pub target: Option<OpenTarget>,

    /// Waits for the command to complete before proceeding.
    #[clap(short = 'w', long)]
    pub wait: bool,

    /// Closes the editor after opening the project.
    #[clap(short = 'Q', long)]
    pub quit: bool,

    /// Suppresses ucom messages.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Shows the command to be run without actually executing it.
    #[clap(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments to be passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

#[derive(Args)]
pub struct BuildArguments {
    /// Specifies the target platform for the build.
    #[arg(value_enum, env = ENV_BUILD_TARGET)]
    pub target: BuildOpenTarget,

    /// Defines the project's directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Sets the output directory for the build. If omitted, the build is placed in <PROJECT_DIR>/Builds/<TARGET>.
    #[arg(
        short = 'o',
        long = "output",
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::FilePath
    )]
    pub build_path: Option<PathBuf>,

    /// Sets the build options. Multiple options can be combined by separating them with spaces.
    #[arg(num_args(0..), short = 'O', long, value_name = "OPTION", default_value="none")]
    pub build_options: Vec<BuildOptions>,

    /// Removes directories from the output directory not needed for distribution.
    #[clap(short = 'C', long)]
    pub clean: bool,

    /// Determines the method of build script injection.
    #[arg(short = 'i', long, value_name = "METHOD", default_value = "auto")]
    pub inject: InjectAction,

    /// Defines the build mode.
    #[arg(short = 'm', long, value_name = "MODE", default_value = "batch")]
    pub mode: BuildMode,

    /// Specifies the static method in the Unity project used for building the project.
    #[arg(
        short = 'f',
        long,
        value_name = "FUNCTION",
        default_value = "Ucom.UnityBuilder.Build"
    )]
    pub build_function: String,

    /// Designates the log file for Unity's build output. By default, log is written to the project's `Logs` directory.
    #[arg(short = 'l', long, value_name = "FILE")]
    pub log_file: Option<PathBuf>,

    /// Suppresses build log output to stdout.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Displays the command to be run without actually executing it.
    #[clap(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments to be passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ListType {
    /// Lists the installed Unity versions.
    Installed,
    /// Displays installed Unity versions and checks for online updates.
    Updates,
    /// Shows the latest available Unity versions.
    Latest,
    /// Shows all available Unity versions.
    All,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum PackagesInfoLevel {
    /// No package information is displayed.
    #[value(name = "none")]
    None,

    /// Shows non-Unity packages only.
    #[value(name = "no-unity")]
    ExcludingUnity,

    /// Additionally includes information for packages from the Unity registry.
    #[value(name = "inc-unity")]
    IncludingUnity,

    /// Displays all package information including built-in packages and dependencies.
    #[value(name = "all")]
    All,
}

impl Display for PackagesInfoLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PackagesInfoLevel::None => write!(f, "none"),
            PackagesInfoLevel::ExcludingUnity => write!(f, "no-unity"),
            PackagesInfoLevel::IncludingUnity => write!(f, "inc-unity"),
            PackagesInfoLevel::All => write!(f, "all"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum InjectAction {
    /// Inject a build script if none exists, and remove it post-build.
    Auto,
    /// Inject a build script into the project and retain it post-build.
    Persistent,
    /// Use the existing build script in the project, without any injection.
    Off,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BuildMode {
    /// Execute build in 'batch' mode and await completion.
    #[value(name = "batch")]
    Batch,

    /// Execute build in 'batch' mode without utilizing the graphics device, and await completion.
    #[value(name = "batch-nogfx")]
    BatchNoGraphics,

    /// Execute build within the editor and terminate post-build.
    #[value(name = "editor-quit")]
    EditorQuit,

    /// Execute build within the editor, keeping it open post-build. Useful for debugging.
    #[value(name = "editor")]
    Editor,
}

/// The build target to open the project with.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum OpenTarget {
    Standalone,
    #[value(name = "win32")]
    Win,
    #[value(name = "win64")]
    Win64,
    #[value(name = "macos")]
    OSXUniversal,
    #[value(name = "linux64")]
    Linux64,
    #[value(name = "ios")]
    iOS,
    #[value(name = "android")]
    Android,
    #[value(name = "webgl")]
    WebGL,
    #[value(name = "winstore")]
    WindowsStoreApps,
    #[value(name = "tvos")]
    tvOS,
}

impl Display for OpenTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The build target to open the project to build with.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum BuildOpenTarget {
    #[value(name = "win32")]
    Win,
    #[value(name = "win64")]
    Win64,
    #[value(name = "macos")]
    OSXUniversal,
    #[value(name = "linux64")]
    Linux64,
    #[value(name = "ios")]
    iOS,
    #[value(name = "android")]
    Android,
    #[value(name = "webgl")]
    WebGL,
}

impl Display for BuildOpenTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The build target to pass to the Unity build script.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum BuildScriptTarget {
    StandaloneOSX,
    StandaloneWindows,
    StandaloneWindows64,
    StandaloneLinux64,
    iOS,
    Android,
    WebGL,
}

impl Display for BuildScriptTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<BuildOpenTarget> for BuildScriptTarget {
    fn from(target: BuildOpenTarget) -> Self {
        match target {
            BuildOpenTarget::Win => Self::StandaloneWindows,
            BuildOpenTarget::Win64 => Self::StandaloneWindows64,
            BuildOpenTarget::OSXUniversal => Self::StandaloneOSX,
            BuildOpenTarget::Linux64 => Self::StandaloneLinux64,
            BuildOpenTarget::iOS => Self::iOS,
            BuildOpenTarget::Android => Self::Android,
            BuildOpenTarget::WebGL => Self::WebGL,
        }
    }
}

/// Building options. Multiple options can be combined together.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BuildOptions {
    /// Perform the specified build without any special settings or extra tasks.
    None = 0,

    /// Build a development version of the player.
    Development = 1,

    /// Run the built player.
    AutoRunPlayer = 4,

    /// Show the built player.
    ShowBuiltPlayer = 8,

    /// Build a compressed asset bundle that contains streamed Scenes loadable with the UnityWebRequest class.
    BuildAdditionalStreamedScenes = 16, // 0x00000010

    /// Used when building Xcode (iOS) or Eclipse (Android) projects.
    AcceptExternalModificationsToPlayer = 32, // 0x00000020

    /// Clear all cached build results, resulting in a full rebuild of all scripts and all player data.
    CleanBuildCache = 128, // 0x00000080

    /// Start the player with a connection to the profiler in the editor.
    ConnectWithProfiler = 256, // 0x00000100

    /// Allow script debuggers to attach to the player remotely.
    AllowDebugging = 512, // 0x00000200

    /// Symlink sources when generating the project.
    /// This is useful if you're changing source files inside the generated project
    /// and want to bring the changes back into your Unity project or a package.
    SymlinkSources = 1024, // 0x00000400

    /// Don't compress the data when creating the asset bundle.
    UncompressedAssetBundle = 2048, // 0x00000800

    /// Sets the Player to connect to the Editor.
    ConnectToHost = 4096, // 0x00001000

    /// Determines if the player should be using the custom connection ID.
    CustomConnectionId = 8192, // 0x00002000

    /// Only build the scripts in a Project.
    BuildScriptsOnly = 3276, // 0x00008000

    /// Patch a Development app package rather than completely rebuilding it.
    /// Supported platforms: Android.
    PatchPackage = 65536, // 0x00010000

    /// Use chunk-based LZ4 compression when building the Player.
    CompressWithLz4 = 262144, // 0x00040000

    /// Use chunk-based LZ4 high-compression when building the Player.
    CompressWithLz4HC = 524288, // 0x00080000

    /// Do not allow the build to succeed if any errors are reporting during it.
    StrictMode = 2097152, // 0x00200000

    /// Build will include Assemblies for testing.
    IncludeTestAssemblies = 4194304, // 0x00400000

    /// Will force the buildGUID to all zeros.
    NoUniqueIdentifier = 8388608, // 0x00800000

    /// Sets the Player to wait for player connection on player start.
    WaitForPlayerConnection = 33554432, // 0x02000000

    /// Enables code coverage.
    /// You can use this as a complimentary way of enabling code coverage on platforms that do not
    /// support command line arguments.
    EnableCodeCoverage = 67108864, // 0x04000000

    /// Enables Deep Profiling support in the player.
    EnableDeepProfilingSupport = 268435456, // 0x10000000

    /// Generates more information in the BuildReport.
    DetailedBuildReport = 536870912, // 0x20000000

    /// Enable Shader Livelink support.
    ShaderLivelinkSupport = 1073741824, // 0x40000000
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Template {
    /// The C# script injected into the project when building.
    BuildScript,

    /// The .gitignore file for newly created projects.
    GitIgnore,

    /// The .gitattributes file for newly created projects.
    GitAttributes,
}

impl Template {
    pub fn content(self) -> &'static str {
        match self {
            Self::BuildScript => include_str!("commands/include/UnityBuilder.cs"),
            Self::GitIgnore => include_str!("commands/include/unity-gitignore.txt"),
            Self::GitAttributes => include_str!("commands/include/unity-gitattributes.txt"),
        }
    }
}
