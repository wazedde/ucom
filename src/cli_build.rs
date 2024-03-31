use std::path::PathBuf;

use clap::{Args, ValueEnum};
use strum::{AsRefStr, Display};

#[derive(Args)]
pub(crate) struct BuildArguments {
    /// Specifies the target platform for the build.
    #[arg(value_enum, env = crate::cli::ENV_BUILD_TARGET)]
    pub(crate) target: BuildOpenTarget,

    /// Defines the project's directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub(crate) project_dir: PathBuf,

    /// Sets the output directory for the build.
    /// If omitted, the build is placed in <PROJECT_DIR>/Builds/<TYPE>/<TARGET>.
    #[arg(
        short = 'o',
        long = "output",
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::FilePath
    )]
    pub(crate) build_path: Option<PathBuf>,

    /// Sets the output type for the build.
    ///
    /// This is mainly a flag used in the output directory; it doesn't dictate the physical type of build.
    /// Ignored if `--output` is set.
    #[arg(
        short = 't',
        long = "type",
        value_name = "TYPE",
        default_value = "release"
    )]
    pub(crate) output_type: BuildOutputType,

    /// Run the built player.
    ///
    /// Same as `--build-options auto-run-player`.
    #[arg(short = 'r', long("run"))]
    pub(crate) run_player: bool,

    /// Build a development version of the player.
    ///
    /// Same as `--build-options development`.
    #[arg(short = 'd', long("development"))]
    pub(crate) development_build: bool,

    /// Show the built player.
    ///
    /// Same as `--build-options show-built-player`.
    #[arg(short = 'S', long("show"))]
    pub(crate) show_built_player: bool,

    /// Allow script debuggers to attach to the player remotely.
    ///
    /// Same as `--build-options allow-debugging`.
    #[arg(short = 'D', long("debugging"))]
    pub(crate) allow_debugging: bool,

    /// Start the player with a connection to the profiler in the editor.
    ///
    /// Same as `--build-options connect-with-profiler`.
    #[arg(short = 'p', long("profiling"))]
    pub(crate) connect_with_profiler: bool,

    /// Enables Deep Profiling support in the player.
    ///
    /// Same as `--build-options enable-deep-profiling-support`.
    #[arg(short = 'P', long("deep-profiling"))]
    pub(crate) deep_profiling: bool,

    /// Sets the Player to connect to the Editor.
    ///
    /// Same as `--build-options connect-to-host`.
    #[arg(short = 'H', long("connect-host"))]
    pub(crate) connect_to_host: bool,

    /// Sets the build options. Multiple options can be combined by separating them with spaces.
    #[arg(num_args(0..), short = 'O', long, value_name = "OPTION", default_value = "none")]
    pub(crate) build_options: Vec<BuildOptions>,

    /// A string to be passed directly to functions tagged with the UcomPreProcessBuild attribute.
    ///
    /// Use it to pass custom arguments to your own C# build scripts before the project is built,
    /// like e.g., a release, debug or test build tag or a version number.
    /// This requires the use of ucom's injected build script as it passes the arguments through.
    #[arg(short = 'a', long, value_name = "STRING")]
    pub(crate) build_args: Option<String>,

    /// Removes directories from the output directory not needed for distribution.
    #[arg(short = 'C', long)]
    pub(crate) clean: bool,

    /// Determines the method of build script injection.
    #[arg(short = 'i', long, value_name = "METHOD", default_value = "auto")]
    pub(crate) inject: InjectAction,

    /// Defines the build mode.
    #[arg(short = 'm', long, value_name = "MODE", default_value = "batch")]
    pub(crate) mode: BuildMode,

    /// Specifies the static method in the Unity project used for building the project.
    #[arg(
        short = 'f',
        long,
        value_name = "FUNCTION",
        default_value = "Ucom.UnityBuilder.Build"
    )]
    pub(crate) build_function: String,

    /// Designates the log file for Unity's build output. By default, log is written to the project's `Logs` directory.
    #[arg(short = 'l', long, value_name = "FILE")]
    pub(crate) log_file: Option<PathBuf>,

    /// Suppresses build log output to stdout.
    #[arg(short = 'q', long)]
    pub(crate) quiet: bool,

    /// Displays the command to be run without actually executing it.
    #[arg(short = 'n', long)]
    pub(crate) dry_run: bool,

    /// A list of arguments to be passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub(crate) args: Option<Vec<String>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub(crate) enum InjectAction {
    /// Inject a build script if none exists, and remove it post-build.
    Auto,
    /// Inject a build script into the project and retain it post-build.
    Persistent,
    /// Use the existing build script in the project, without any injection.
    Off,
}

#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub(crate) enum BuildOutputType {
    /// Build output is written to the `Builds/Release` directory.
    Release,
    /// Build output is written to the `Builds/Debug` directory.
    Debug,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub(crate) enum BuildMode {
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
#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub(crate) enum OpenTarget {
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

/// The build target to open the project to build with.
#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub(crate) enum BuildOpenTarget {
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

/// The build target to pass to the Unity build script.
#[derive(Display, AsRefStr, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub(crate) enum BuildScriptTarget {
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

/// Building options. Multiple options can be combined.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub(crate) enum BuildOptions {
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
