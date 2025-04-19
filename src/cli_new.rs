use std::path::PathBuf;

use crate::cli_build::OpenTarget;
use clap::Args;

#[derive(Args)]
pub struct NewArguments {
    /// Specify the Unity version to use for the new project.
    /// Accepts a full version (e.g., '2022.3.5f1') or a prefix (e.g., '2021', '2022.3').
    /// A prefix will select the latest installed version matching that prefix. Required.
    #[arg(short = 'u', long = "unity", value_name = "VERSION", required = true)]
    pub version_pattern: String,

    /// Path and name for the new project directory. This directory must not exist yet. Required.
    #[arg(
        required = true,
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::DirPath
    )]
    pub project_dir: PathBuf,

    /// Set the initial active build target for the new project (e.g., win64, android).
    #[arg(short = 't', long, value_name = "NAME")]
    pub target: Option<OpenTarget>,

    /// Add helper scripts ('UnityBuilder.cs', 'EditorMenu.cs') for building via ucom or the Editor menu.
    ///
    /// Files are placed in 'Assets/Plugins/Ucom/Editor/'.
    #[arg(long)]
    pub add_builder_menu: bool,

    /// Initialize a Git repository, configure Git LFS, and add a Unity-specific '.gitattributes' file.
    #[arg(long = "lfs")]
    pub include_lfs: bool,

    /// Skip initializing a Git repository for the new project.
    #[arg(long, conflicts_with = "include_lfs")] // Conflict remains valid
    pub no_git: bool,

    /// Wait for the initial Unity editor process (used for project creation) to exit before returning.
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Automatically close the Unity editor immediately after the project creation process finishes.
    #[arg(short = 'Q', long)]
    pub quit: bool,

    /// Suppress informational messages from ucom during project creation.
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show the command that would be executed without actually running it.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Additional arguments to pass directly to the Unity editor executable during project creation.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub args: Option<Vec<String>>,
}
