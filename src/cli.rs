use std::path::PathBuf;

use clap::{Args, ValueEnum};
use strum::Display;

use crate::cli_add::AddArguments;
use crate::cli_build::{BuildArguments, OpenTarget};
use crate::cli_new::NewArguments;
use crate::cli_run::RunArguments;
use crate::cli_test::TestArguments;

pub const ENV_BUILD_TARGET: &str = "UCOM_BUILD_TARGET";
pub const ENV_PACKAGE_LEVEL: &str = "UCOM_PACKAGE_LEVEL";

/// Unity Commander: A command-line interface for managing Unity projects.
#[derive(clap::Parser)]
#[command(author, version, about)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Suppress colored output in the terminal.
    #[arg(long, short = 'n')]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// List installed or available Unity versions.
    #[command(visible_alias = "ls")]
    List {
        /// Specify the type of versions to list.
        #[arg(value_enum, default_value = "installed")]
        list_type: ListType,

        /// Filter versions by a prefix (e.g., '2021', '2022.3').
        #[arg(short = 'u', long = "unity", value_name = "VERSION")]
        version_filter: Option<String>,

        /// Force download of release data from the Unity API, bypassing cache.
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Install a specific Unity version.
    #[command()]
    Install {
        /// Version to install (e.g., '2023.1', '2021.1.0f1').
        #[arg(value_name = "VERSION")]
        version: String,
    },

    /// Display information about a Unity project.
    #[command(visible_alias = "i")]
    Info {
        /// Path to the Unity project directory. Defaults to the current directory.
        #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
        project_dir: PathBuf,

        /// Install the project's required Unity version if it's not already installed.
        #[arg(long)]
        install_required: bool,

        /// Recursively search directories for the Unity project.
        #[arg(short = 'R', long)]
        recursive: bool,

        /// Set the level of detail for displaying package information.
        #[arg(short = 'p', long, default_value = "no-unity", env = ENV_PACKAGE_LEVEL)]
        packages: PackagesInfoLevel,

        /// Generate a Markdown report with available updates.
        #[arg(short = 'r', long)]
        report: bool,
    },

    /// Check for newer available Unity versions suitable for a project.
    #[command(visible_alias = "u")]
    Updates {
        /// Path to the Unity project directory. Defaults to the current directory.
        #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
        project_dir: PathBuf,

        /// Install the latest suitable Unity version if it's not already installed.
        #[arg(long)]
        install_latest: bool,

        /// Generate a Markdown report of applicable release notes.
        #[arg(short = 'r', long)]
        report: bool,
    },

    /// Create a new Unity project, optionally initializing a Git repository.
    #[command()]
    New(NewArguments),

    /// Open a Unity project in the editor.
    #[command(visible_alias = "o")]
    Open(OpenArguments),

    /// Build a Unity project for a specified target platform.
    #[command(visible_alias = "b")]
    Build(BuildArguments),

    /// Run tests within a Unity project.
    #[command(visible_alias = "t")]
    Test(TestArguments),

    /// Run the Unity editor with custom command-line arguments.
    #[command(visible_alias = "r")]
    Run(RunArguments),

    /// Add a helper script or configuration file to the project.
    Add(AddArguments),

    /// Manage the download cache for Unity release data.
    ///
    /// By default, cached files expire after one hour. The system will
    /// automatically re-download required files after this timeout.
    ///
    /// Control caching behavior with the `UCOM_ENABLE_CACHE` environment variable.
    /// Set it to 'false' to disable caching and always download fresh data.
    #[command()]
    Cache {
        /// Action to perform on the cache.
        #[arg(value_enum)]
        action: CacheAction,
    },
}

#[derive(Args)]
pub struct OpenArguments {
    /// Path to the Unity project directory. Defaults to the current directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Upgrade the project to a newer Unity version before opening.
    /// If no version is specified, uses the latest installed version matching the project's `major.minor`.
    /// A version prefix (e.g., '2021') selects the latest installed version in that release series.
    #[arg(long = "upgrade", value_name = "VERSION", num_args = 0..=1, require_equals = true, default_missing_value = None)]
    pub upgrade_version: Option<Option<String>>, // Outer Option: present?, Inner Option: value provided?

    /// Set the active build target.
    #[arg(short = 't', long, value_name = "NAME")]
    pub target: Option<OpenTarget>,

    /// Wait for the Unity editor process to exit before the command returns.
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Automatically close the Unity editor after the project load completes.
    #[arg(short = 'Q', long)]
    pub quit: bool,

    /// Suppress informational messages from ucom before launching Unity.
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show the command that would be executed without actually running it.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Additional arguments to pass directly to the Unity editor executable.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Display, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ListType {
    /// List Unity versions currently installed on the system.
    Installed,
    /// List installed versions and check for available updates for each.
    Updates,
    /// List the latest available version for each major/minor release series.
    Latest,
    /// List all known available Unity versions from the release data.
    All,
}

#[derive(Display, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum PackagesInfoLevel {
    /// Do not display any package information.
    #[value(name = "none")]
    #[strum(serialize = "none")]
    None,

    /// Display information only for non-Unity packages (e.g., custom, third-party).
    #[value(name = "no-unity")]
    #[strum(serialize = "no-unity")]
    ExcludingUnity,

    /// Display information for non-Unity and Unity registry packages.
    #[value(name = "inc-unity")]
    #[strum(serialize = "inc-unity")]
    IncludingUnity,

    /// Display information for all packages, including built-in and dependencies.
    #[value(name = "all")]
    #[strum(serialize = "all")]
    All,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum CacheAction {
    /// Remove all cached download files.
    Clear,
    /// Display a list of currently cached files.
    List,
}
