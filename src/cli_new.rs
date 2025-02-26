use std::path::PathBuf;

use crate::cli_build::OpenTarget;
use clap::Args;

#[derive(Args)]
pub struct NewArguments {
    /// Unity version for new project (e.g. '2021' uses latest 2021.x.y)
    #[arg(short = 'u', long = "unity", value_name = "VERSION")]
    pub version_pattern: String,

    /// Target directory (must not exist)
    #[arg(
        required = true,
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::DirPath
    )]
    pub project_dir: PathBuf,

    /// Set active build target
    #[arg(short = 't', long, value_name = "NAME")]
    pub target: Option<OpenTarget>,

    /// Add build menu script to project
    ///
    /// Adds both EditorMenu.cs and UnityBuilder.cs scripts
    /// to Assets/Plugins/Ucom/Editor directory
    #[arg(long)]
    pub add_builder_menu: bool,

    /// Initialize Git LFS with Unity-specific attributes
    #[arg(long = "lfs")]
    pub include_lfs: bool,

    /// Skip Git repository initialization
    #[arg(long, conflicts_with = "include_lfs")]
    pub no_git: bool,

    /// Wait for Unity to exit before returning
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Close editor after project creation
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
