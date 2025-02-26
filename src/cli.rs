use std::path::PathBuf;

use clap::{Args, ValueEnum};
use strum::Display;

use crate::cli_add::AddArguments;
use crate::cli_build::{BuildArguments, OpenTarget};
use crate::cli_new::NewArguments;
use crate::cli_run::RunArguments;
use crate::cli_test::TestArguments;

pub const ENV_EDITOR_DIR: &str = "UCOM_EDITOR_DIR";
pub const ENV_BUILD_TARGET: &str = "UCOM_BUILD_TARGET";
pub const ENV_PACKAGE_LEVEL: &str = "UCOM_PACKAGE_LEVEL";

/// Unity Commander: A command-line interface for Unity projects.
#[derive(clap::Parser)]
#[command(author, version, about)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Disable colored output
    #[arg(long, short = 'D')]
    pub disable_color: bool,

    #[command(subcommand)]
    pub command: Option<Action>,
}

#[derive(clap::Subcommand)]
pub enum Action {
    /// List installed Unity versions
    #[command(visible_alias = "l")]
    List {
        /// Specify what to list
        #[arg(value_enum, default_value = "installed")]
        list_type: ListType,

        /// Filter by Unity version prefix (e.g. '2021')
        #[arg(short = 'u', long = "unity", value_name = "VERSION")]
        version_filter: Option<String>,

        /// Force downloading release data from Unity API
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Install Unity version
    #[command(visible_alias = "I")]
    Install {
        /// Version to install (prefix like '2023.1' or full version like '2021.1.0f1')
        #[arg(value_name = "VERSION")]
        version: String,
    },

    /// Display project information
    #[command(visible_alias = "i")]
    Info {
        /// Project directory path
        #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
        project_dir: PathBuf,

        /// Install required Unity version if not present
        #[arg(short = 'I', long)]
        install: bool,

        /// Recursively search for Unity projects
        #[arg(short = 'R', long)]
        recursive: bool,

        /// Package information detail level
        #[arg(short = 'p', long, default_value = "no-unity", env = ENV_PACKAGE_LEVEL)]
        packages: PackagesInfoLevel,
    },

    /// Check for Unity updates
    #[command(visible_alias = "c")]
    Check {
        /// Project directory path
        #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
        project_dir: PathBuf,

        /// Install latest Unity version if not present
        #[arg(short = 'I', long)]
        install: bool,

        /// Generate Markdown report of release notes
        #[arg(short = 'r', long)]
        report: bool,
    },

    /// Create new Unity project and Git repository
    #[command(visible_alias = "n")]
    New(NewArguments),

    /// Open Unity project in the editor
    #[command(visible_alias = "o")]
    Open(OpenArguments),

    /// Build Unity project
    #[command(visible_alias = "b")]
    Build(BuildArguments),

    /// Run project tests
    #[command(visible_alias = "t")]
    Test(TestArguments),

    /// Run Unity with specified arguments
    #[command(visible_alias = "r")]
    Run(RunArguments),

    /// Add helper script or configuration file
    Add(AddArguments),

    /// Manage download cache
    ///
    /// By default, cached files expire after one hour.
    /// The system will re-download required files after this timeout.
    ///
    /// Control caching with UCOM_ENABLE_CACHE environment variable.
    /// Set to 'false' to disable caching and always download fresh data.
    #[command()]
    Cache {
        #[arg(value_enum)]
        action: CacheAction,
    },
}

#[derive(Args)]
pub struct OpenArguments {
    /// Project directory path
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Upgrade project's Unity version
    /// If no version specified, uses latest in project's major.minor range.
    /// Version prefix like '2021' selects latest installed in that range.
    #[arg(short = 'U', long = "upgrade", value_name = "VERSION")]
    pub upgrade_version: Option<Option<String>>,

    /// Set active build target
    #[arg(short = 't', long, value_name = "NAME")]
    pub target: Option<OpenTarget>,

    /// Wait for Unity to exit before returning
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Close editor after opening project
    #[arg(short = 'Q', long)]
    pub quit: bool,

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

#[derive(Debug, Display, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ListType {
    /// List installed Unity versions
    Installed,
    /// Show installed versions and check for updates
    Updates,
    /// Show latest available Unity versions
    Latest,
    /// Show all available Unity versions
    All,
}

#[derive(Display, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum PackagesInfoLevel {
    /// No package information
    #[value(name = "none")]
    #[strum(serialize = "none")]
    None,

    /// Non-Unity packages only
    #[value(name = "no-unity")]
    #[strum(serialize = "no-unity")]
    ExcludingUnity,

    /// Include Unity registry packages
    #[value(name = "inc-unity")]
    #[strum(serialize = "inc-unity")]
    IncludingUnity,

    /// All packages including built-in and dependencies
    #[value(name = "all")]
    #[strum(serialize = "all")]
    All,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum CacheAction {
    /// Remove all cached files
    Clear,
    /// Show list of cached files
    List,
}
