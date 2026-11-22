use std::path::PathBuf;

use clap::{Args, ValueEnum};
use strum::Display;

use crate::cli::ENV_PACKAGE_LEVEL;

#[derive(Args)]
pub struct InfoArguments {
    /// Path to the Unity project directory. Defaults to the current directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Install the project's required Unity version if it's not already installed.
    #[arg(long)]
    pub install_required: bool,

    /// Recursively search directories for the Unity project.
    #[arg(short = 'R', long)]
    pub recursive: bool,

    /// Set the level of detail for displaying package information.
    #[arg(short = 'p', long, default_value = "no-unity", env = ENV_PACKAGE_LEVEL)]
    pub packages: PackagesInfoLevel,

    /// Generate a Markdown report with available updates.
    #[arg(short = 'r', long)]
    pub report: bool,
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
