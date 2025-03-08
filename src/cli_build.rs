use std::path::PathBuf;

use clap::{Args, ValueEnum};
use strum::{AsRefStr, Display};

#[derive(Args)]
pub struct BuildArguments {
    /// Target platform for build
    #[arg(value_enum, env = crate::cli::ENV_BUILD_TARGET)]
    pub target: BuildOpenTarget,

    /// Project directory path
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Output directory for build
    /// [default: <PROJECT_DIR>/Builds/<TYPE>/<TARGET>]
    #[arg(
        short = 'o',
        long = "output",
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::FilePath
    )]
    pub build_path: Option<PathBuf>,

    /// Output type for build directory naming
    ///
    /// Used in output directory path structure
    /// Ignored if --output is set
    #[arg(
        short = 't',
        long = "type",
        value_name = "TYPE",
        default_value = "release"
    )]
    pub output_type: BuildOutputType,

    /// Run built player
    ///
    /// Same as --build-options auto-run-player
    #[arg(short = 'r', long = "run")]
    pub run_player: bool,

    /// Build development version
    ///
    /// Same as --build-options development
    #[arg(short = 'd', long = "development")]
    pub development_build: bool,

    /// Show built player
    ///
    /// Same as --build-options show-built-player
    #[arg(short = 'S', long = "show")]
    pub show_built_player: bool,

    /// Allow remote script debugging
    ///
    /// Same as --build-options allow-debugging
    #[arg(short = 'D', long = "debugging")]
    pub allow_debugging: bool,

    /// Connect to editor profiler
    ///
    /// Same as --build-options connect-with-profiler
    #[arg(short = 'p', long = "profiling")]
    pub connect_with_profiler: bool,

    /// Enable deep profiling support
    ///
    /// Same as --build-options enable-deep-profiling-support
    #[arg(short = 'P', long = "deep-profiling")]
    pub deep_profiling: bool,

    /// Connect player to editor
    ///
    /// Same as --build-options connect-to-host
    #[arg(short = 'H', long = "connect-host")]
    pub connect_to_host: bool,

    /// Set Unity build options (space-separated)
    #[arg(num_args(0..), short = 'O', long, value_name = "OPTION", default_value = "none")]
    pub build_options: Vec<BuildOptions>,

    /// Custom argument string for UcomPreProcessBuild
    ///
    /// Passed to functions with UcomPreProcessBuild attribute
    /// Useful for version numbers or build configuration flags
    /// Requires ucom's injected build script
    #[arg(short = 'a', long, value_name = "STRING")]
    pub build_args: Option<String>,

    /// Remove unused files from output directory
    #[arg(short = 'C', long)]
    pub clean: bool,

    /// Build script injection method
    #[arg(short = 'i', long, value_name = "METHOD", default_value = "auto")]
    pub inject: InjectAction,

    /// Build mode
    #[arg(short = 'm', long, value_name = "MODE", default_value = "batch")]
    pub mode: BuildMode,

    /// Static build method in project
    #[arg(
        short = 'f',
        long,
        value_name = "FUNCTION",
        default_value = "Ucom.UnityBuilder.Build"
    )]
    pub build_function: String,

    /// Log file for Unity build output
    /// [default: <PROJECT_DIR>/Logs directory]
    #[arg(short = 'l', long, value_name = "FILE")]
    pub log_file: Option<PathBuf>,

    /// Suppress build log output
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show command without executing
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Arguments to pass directly to Unity
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum InjectAction {
    /// Inject build script temporarily
    Auto,
    /// Inject build script permanently
    Persistent,
    /// Use existing build script only
    Off,
}

#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BuildOutputType {
    /// Output to Builds/Release directory
    Release,
    /// Output to Builds/Debug directory
    Debug,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BuildMode {
    /// Build in batch mode
    #[value(name = "batch")]
    Batch,

    /// Build in batch mode without graphics
    #[value(name = "batch-nogfx")]
    BatchNoGraphics,

    /// Build in editor and quit
    #[value(name = "editor-quit")]
    EditorQuit,

    /// Build in editor and stay open
    #[value(name = "editor")]
    Editor,
}

/// Build target for Unity editor
#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
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

/// Build target platform
#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
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

/// Target for Unity build script
#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
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

/// Build options flags
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum BuildOptions {
    /// Default build with no special settings
    None = 0,

    /// Build development version
    Development = 1,

    /// Run built player
    AutoRunPlayer = 4,

    /// Show built player
    ShowBuiltPlayer = 8,

    /// Build compressed asset bundle with streamed scenes
    BuildAdditionalStreamedScenes = 16, // 0x00000010

    /// Used for Xcode (iOS) or Eclipse (Android) projects
    AcceptExternalModificationsToPlayer = 32, // 0x00000020

    /// Force full rebuild of all scripts and player data
    CleanBuildCache = 128, // 0x00000080

    /// Connect to profiler in editor
    ConnectWithProfiler = 256, // 0x00000100

    /// Allow remote script debugging
    AllowDebugging = 512, // 0x00000200

    /// Symlink sources for project generation
    SymlinkSources = 1024, // 0x00000400

    /// Skip asset bundle compression
    UncompressedAssetBundle = 2048, // 0x00000800

    /// Connect player to editor
    ConnectToHost = 4096, // 0x00001000

    /// Use custom connection ID
    CustomConnectionId = 8192, // 0x00002000

    /// Build only scripts
    BuildScriptsOnly = 3276, // 0x00008000

    /// Patch Android development package
    PatchPackage = 65536, // 0x00010000

    /// Use LZ4 compression
    CompressWithLz4 = 262144, // 0x00040000

    /// Use LZ4 high-compression
    CompressWithLz4HC = 524288, // 0x00080000

    /// Fail build on any errors
    StrictMode = 2097152, // 0x00200000

    /// Include test assemblies
    IncludeTestAssemblies = 4194304, // 0x00400000

    /// Use zero GUID
    NoUniqueIdentifier = 8388608, // 0x00800000

    /// Wait for player connection on start
    WaitForPlayerConnection = 33554432, // 0x02000000

    /// Enable code coverage
    EnableCodeCoverage = 67108864, // 0x04000000

    /// Enable deep profiling support
    EnableDeepProfilingSupport = 268435456, // 0x10000000

    /// Generate detailed build report
    DetailedBuildReport = 536870912, // 0x20000000

    /// Enable shader livelink
    ShaderLivelinkSupport = 1073741824, // 0x40000000
}
