use std::path::PathBuf;

use clap::Args;

use crate::cli_build::OpenTarget;
use crate::unity::VersionPattern;

#[derive(Args)]
pub struct OpenArguments {
    /// Path to the Unity project directory. Defaults to the current directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Upgrade the project to a newer Unity version before opening.
    /// If no version is specified, uses the latest installed version matching the project's `major.minor`.
    /// A version pattern (e.g., '2022', '2022.3', '2022.3.*') selects the latest installed version that matches it.
    #[arg(long = "upgrade", value_name = "VERSION", num_args = 0..=1, require_equals = true, default_missing_value = None)]
    pub upgrade_version: Option<Option<VersionPattern>>, // Outer Option: present?, Inner Option: value provided?

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
