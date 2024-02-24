use clap::Args;

#[derive(Args)]
pub struct RunArguments {
    /// Specifies the Unity version to run. For example, '2021' runs the latest-installed 2021.x.y version.
    #[arg(
        short = 'u',
        long = "unity",
        value_name = "VERSION",
        env = crate::cli::ENV_DEFAULT_VERSION,
    )]
    pub version_pattern: Option<String>,

    /// Waits for the command to complete before proceeding.
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Suppresses ucom messages.
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Displays the command to be run without actually executing it.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// A list of arguments to be passed directly to Unity.
    #[arg(last = true, value_name = "UNITY_ARGS", required = true)]
    pub args: Option<Vec<String>>,
}
