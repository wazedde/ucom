use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use clap::{Args, ValueEnum};

pub const ENV_EDITOR_DIR: &str = "UCOM_EDITOR_DIR";
pub const ENV_DEFAULT_VERSION: &str = "UCOM_DEFAULT_VERSION";
pub const ENV_BUILD_TARGET: &str = "UCOM_BUILD_TARGET";
pub const ENV_PACKAGE_LEVEL: &str = "UCOM_PACKAGE_LEVEL";

/// Unity Commander, a command line interface for Unity projects.
#[derive(clap::Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Display the build script that is injected into the project.
    #[arg(long)]
    pub injected_script: bool,

    /// Disable colored output.
    #[arg(long, short = 'D')]
    pub disable_color: bool,

    #[command(subcommand)]
    pub command: Option<Action>,
}

#[derive(clap::Subcommand)]
pub enum Action {
    /// Shows a list of the installed Unity versions.
    #[command(visible_alias = "l")]
    List {
        /// What to list.
        #[arg(value_enum, default_value = "installed")]
        list_type: ListType,

        /// The Unity versions to list. You can specify a partial version; e.g. 2021 will list all
        /// the 2021.x.y versions you have installed on your system.
        #[arg(short = 'u', long = "unity", value_name = "VERSION")]
        version_pattern: Option<String>,
    },

    /// Shows project information.
    #[command(visible_alias = "i")]
    Info {
        /// The directory of the project.
        #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
        project_dir: PathBuf,

        /// The level of included packages to show.
        #[arg(short='p', long, default_value = "lev1", env = ENV_PACKAGE_LEVEL)]
        packages: PackagesInfoLevel,
    },

    /// Checks on the Unity website for updates to the version used by the project.
    #[command(visible_alias = "c")]
    Check {
        /// The directory of the project.
        #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
        project_dir: PathBuf,

        /// Creates a Markdown report of the releases that are available.
        #[clap(short = 'r', long, value_name = "FILE.md", value_hint = clap::ValueHint::FilePath)]
        create_report: Option<PathBuf>,
    },

    /// Creates a new Unity project and Git repository (uses latest installed version by default)
    #[command(visible_alias = "n")]
    New(NewArguments),

    /// Opens the given Unity project in the Unity Editor
    #[command(visible_alias = "o")]
    Open(OpenArguments),

    /// Builds the given Unity project
    #[command(visible_alias = "b")]
    Build(BuildArguments),

    /// Runs Unity with the givens arguments (uses latest installed version by default)
    #[command(visible_alias = "r")]
    Run(RunArguments),
}

#[derive(Args)]
pub struct RunArguments {
    /// The Unity version to run. You can specify a partial version; e.g. 2021 will match the
    /// latest 2021.x.y version you have installed on your system.
    #[arg(
        short = 'u',
        long = "unity",
        value_name = "VERSION",
        env = ENV_DEFAULT_VERSION,
    )]
    pub version_pattern: Option<String>,

    /// Waits for the command to finish before continuing.
    #[clap(short = 'w', long)]
    pub wait: bool,

    /// Do not print ucom log messages.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Show what would be run, but do not actually run it.
    #[clap(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS", required = true)]
    pub args: Option<Vec<String>>,
}

#[derive(Args)]
pub struct NewArguments {
    /// The Unity version to use for the new project. You can specify a partial version;
    /// e.g. 2021 will match the latest 2021.x.y version you have installed on your system.
    #[arg(
        short = 'u',
        long = "unity",
        value_name = "VERSION",
        env = ENV_DEFAULT_VERSION
    )]
    pub version_pattern: Option<String>,

    /// The directory where the project is created. This directory should not exist yet.
    #[arg(
        required = true,
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::DirPath
    )]
    pub project_dir: PathBuf,

    /// Suppress initializing a new git repository.
    #[clap(long)]
    pub no_git: bool,

    /// Waits for the command to finish before continuing.
    #[clap(short = 'w', long)]
    pub wait: bool,

    /// Quits the editor after the project has been created.
    #[clap(short = 'Q', long)]
    pub quit: bool,

    /// Do not print ucom log messages.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Show what would be run, but do not actually run it.
    #[clap(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

#[derive(Args)]
pub struct OpenArguments {
    /// The directory of the project.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// The Unity version to open the project with. Use it to open a project with a newer
    /// Unity version. You can specify a partial version; e.g. 2021 will match the latest
    /// 2021.x.y version you have installed on your system.
    #[arg(short = 'u', long = "unity", value_name = "VERSION")]
    pub version_pattern: Option<String>,

    /// Waits for the command to finish before continuing.
    #[clap(short = 'w', long)]
    pub wait: bool,

    /// Do not print ucom log messages.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Show what would be run, but do not actually run it.
    #[clap(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

#[derive(Args)]
pub struct BuildArguments {
    /// The target platform to build for.
    #[arg(value_enum, env = ENV_BUILD_TARGET)]
    pub target: Target,

    /// The directory of the project.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// The output directory of the build. When omitted the build will be placed in <PROJECT_DIR>/Builds/<TARGET>.
    #[arg(
        short = 'o',
        long = "output",
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::FilePath
    )]
    pub build_path: Option<PathBuf>,

    /// Building options. Multiple options can be combined together.
    #[arg(num_args(0..), short = 'O', long, value_name = "OPTIONS", default_value="none")]
    pub build_options: Vec<BuildOptions>,

    /// Removes directories from the output directory that should not be included in the build.
    #[clap(short = 'C', long)]
    pub clean: bool,

    /// Build script injection method.
    #[arg(short = 'i', long, value_name = "METHOD", default_value = "auto")]
    pub inject: InjectAction,

    /// Build mode.
    #[arg(short = 'm', long, value_name = "MODE", default_value = "batch")]
    pub mode: BuildMode,

    /// A static method in the Unity project that is called to build the project.
    #[arg(
        short = 'f',
        long,
        value_name = "FUNCTION",
        default_value = "Ucom.UnityBuilder.Build"
    )]
    pub build_function: String,

    /// The log file to write Unity's build output to.
    /// By default the log is written to the `Logs` directory of the project.
    #[arg(short = 'l', long, value_name = "FILE")]
    pub log_file: Option<PathBuf>,

    /// Don't output the build log to stdout.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Show what would be run, but do not actually run it.
    #[clap(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ListType {
    /// List installed Unity versions.
    Installed,
    /// List installed Unity versions and checks for updates online.
    Updates,
    /// Check for the latest Unity versions online.
    Latest,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum PackagesInfoLevel {
    /// Don't show any included packages.
    Lev0,
    /// Show local, non-Unity, packages.
    Lev1,
    /// + packages from the Unity registry.
    Lev2,
    /// + builtin packages and dependencies.
    Lev3,
}

impl Display for PackagesInfoLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum InjectAction {
    /// If there is no build script, inject one and remove it after the build.
    Auto,
    /// Inject the build script into the project and don't remove it afterwards.
    Persistent,
    /// Don't inject the build script and use the one that is already in the project.
    Off,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BuildMode {
    /// Build in batch mode and wait for the build to finish.
    #[value(name = "batch")]
    Batch,

    /// Build in batch mode without the graphics device and wait for the build to finish.
    #[value(name = "batch-nogfx")]
    BatchNoGraphics,

    /// Build in the editor and quit after the build.
    #[value(name = "editor-quit")]
    EditorQuit,

    /// Build in the editor and keep it open (handy for debugging the build process).
    #[value(name = "editor")]
    Editor,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum Target {
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

impl Display for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum BuildTarget {
    StandaloneOSX,
    StandaloneWindows,
    StandaloneWindows64,
    StandaloneLinux64,
    iOS,
    Android,
    WebGL,
}

impl Display for BuildTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<Target> for BuildTarget {
    fn from(target: Target) -> Self {
        match target {
            Target::Win => Self::StandaloneWindows,
            Target::Win64 => Self::StandaloneWindows64,
            Target::OSXUniversal => Self::StandaloneOSX,
            Target::Linux64 => Self::StandaloneLinux64,
            Target::iOS => Self::iOS,
            Target::Android => Self::Android,
            Target::WebGL => Self::WebGL,
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

    // Specified, but doesn't seem to be use: InstallInBuildFolder = 64, // 0x00000040
    /// Clear all cached build results, resulting in a full rebuild of all scripts and all player data.
    CleanBuildCache = 128, // 0x00000080

    /// Start the player with a connection to the profiler in the editor.
    ConnectWithProfiler = 256, // 0x00000100

    /// Allow script debuggers to attach to the player remotely.
    AllowDebugging = 512, // 0x00000200

    /// Symlink sources when generating the project. This is useful if you're changing source files inside the generated project and want to bring the changes back into your Unity project or a package.
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
    CompressWithLz4Hc = 524288, // 0x00080000

    /// Do not allow the build to succeed if any errors are reporting during it.
    StrictMode = 2097152, // 0x00200000

    /// Build will include Assemblies for testing.
    IncludeTestAssemblies = 4194304, // 0x00400000

    /// Will force the buildGUID to all zeros.
    NoUniqueIdentifier = 8388608, // 0x00800000

    /// Sets the Player to wait for player connection on player start.
    WaitForPlayerConnection = 33554432, // 0x02000000

    /// Enables code coverage. You can use this as a complimentary way of enabling code coverage on platforms that do not support command line arguments.
    EnableCodeCoverage = 67108864, // 0x04000000

    /// Enables Deep Profiling support in the player.
    EnableDeepProfilingSupport = 268435456, // 0x10000000

    /// Generates more information in the BuildReport.
    DetailedBuildReport = 536870912, // 0x20000000

    /// Enable Shader Livelink support.
    ShaderLivelinkSupport = 1073741824, // 0x40000000
}
