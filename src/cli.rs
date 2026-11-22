use crate::cli_add::AddArguments;
use crate::cli_build::BuildArguments;
use crate::cli_cache::CacheArguments;
use crate::cli_info::InfoArguments;
use crate::cli_list::ListArguments;
use crate::cli_new::NewArguments;
use crate::cli_open::OpenArguments;
use crate::cli_run::RunArguments;
use crate::cli_test::TestArguments;
use crate::cli_updates::UpdatesArguments;
use crate::unity::VersionPattern;

pub const ENV_BUILD_TARGET: &str = "UCOM_BUILD_TARGET";
pub const ENV_PACKAGE_LEVEL: &str = "UCOM_PACKAGE_LEVEL";

/// Unity Commander: A command-line interface for managing Unity projects.
#[derive(clap::Parser)]
#[command(author, version, about)]
#[command(arg_required_else_help = true)]
pub struct Cli {
    /// Suppress colored output in the terminal. Setting the NO_COLOR environment variable does the same.
    #[arg(long, short = 'n')]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// List installed or available Unity versions.
    #[command(visible_alias = "ls")]
    List(ListArguments),

    /// Install a specific Unity version.
    #[command()]
    Install {
        /// Version to install (e.g., '2021.1.0f1'), or a pattern (e.g., '2023.1', '2022.3.*')
        /// to install the latest matching version.
        ///
        /// Each given component must match as a whole number, so '2022.3.1' does not match
        /// '2022.3.10f1'. Use '*' for wildcards.
        #[arg(value_name = "VERSION")]
        version: VersionPattern,
    },

    /// Display information about a Unity project.
    #[command(visible_alias = "i")]
    Info(InfoArguments),

    /// Check for newer available Unity versions suitable for a project.
    #[command(visible_alias = "u")]
    Updates(UpdatesArguments),

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
    /// Cached files are refreshed after one hour. Use `ucom list -f` to force a full
    /// refresh of the release data, or `ucom cache clear` to remove all cached files.
    /// If the cache cannot be written, ucom continues without it and prints a warning.
    #[command()]
    Cache(CacheArguments),
}
