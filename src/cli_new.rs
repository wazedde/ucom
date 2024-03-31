use std::path::PathBuf;

use crate::cli_build::OpenTarget;
use clap::Args;

#[derive(Args)]
pub(crate) struct NewArguments {
    /// Specifies the Unity version for the new project.
    /// For example, '2021' uses the latest-installed 2021.x.y version.
    #[arg(
        short = 'u',
        long = "unity",
        value_name = "VERSION",
        env = crate::cli::ENV_DEFAULT_VERSION
    )]
    pub(crate) version_pattern: Option<String>,

    /// Defines the directory for creating the project. This directory should not pre-exist.
    #[arg(
        required = true,
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::DirPath
    )]
    pub(crate) project_dir: PathBuf,

    /// Determines the active build target to open the project with.
    #[arg(short = 't', long, value_name = "NAME")]
    pub(crate) target: Option<OpenTarget>,

    /// Adds a build menu script to the project.
    ///
    /// This will add both the `EditorMenu.cs` and `UnityBuilder.cs`
    /// scripts to the project in the `Assets/Plugins/Ucom/Editor` directory.
    #[arg(long)]
    pub(crate) add_builder_menu: bool,

    /// Initializes LFS for the repository and includes a .gitattributes file with Unity-specific LFS settings.
    #[arg(long = "lfs")]
    pub(crate) include_lfs: bool,

    /// Skips initialization of a new Git repository.
    #[arg(long, conflicts_with = "include_lfs")]
    pub(crate) no_git: bool,

    /// Waits for the command to complete before proceeding.
    #[arg(short = 'w', long)]
    pub(crate) wait: bool,

    /// Closes the editor after the project creation.
    #[arg(short = 'Q', long)]
    pub(crate) quit: bool,

    /// Suppresses ucom messages.
    #[arg(short = 'q', long)]
    pub(crate) quiet: bool,

    /// Shows the command to be run without actually executing it.
    #[arg(short = 'n', long)]
    pub(crate) dry_run: bool,

    /// A list of arguments to be passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS")]
    pub(crate) args: Option<Vec<String>>,
}
