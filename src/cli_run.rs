use clap::Args;

#[derive(Args)]
pub struct RunArguments {
    /// Unity version to run (e.g. '2021' for latest 2021.x.y)
    #[arg(short = 'u', long = "unity", value_name = "VERSION")]
    pub version_pattern: String,

    /// Wait for Unity to exit before returning
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Suppress messages
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show command without executing
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Arguments to pass directly to Unity
    #[arg(last = true, value_name = "UNITY_ARGS", required = true)]
    pub args: Option<Vec<String>>,
}
