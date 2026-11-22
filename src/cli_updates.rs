use std::path::PathBuf;

use clap::Args;

#[derive(Args)]
pub struct UpdatesArguments {
    /// Path to the Unity project directory. Defaults to the current directory.
    #[arg(value_name = "DIRECTORY", value_hint = clap::ValueHint::DirPath, default_value = ".")]
    pub project_dir: PathBuf,

    /// Install the latest suitable Unity version if it's not already installed.
    #[arg(long)]
    pub install_latest: bool,

    /// Generate a Markdown report of applicable release notes.
    #[arg(short = 'r', long)]
    pub report: bool,
}
