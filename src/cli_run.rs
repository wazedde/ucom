use clap::Args;

use crate::unity::VersionPattern;

#[derive(Args)]
pub struct RunArguments {
    /// Specify the Unity Editor version to execute.
    /// Accepts a full version (e.g., '2022.3.5f1') or a pattern (e.g., '2022', '2022.3', '2022.3.*').
    /// A pattern selects the latest installed version that matches it; each given component
    /// must match as a whole number. Required.
    #[arg(short = 'u', long = "unity", value_name = "VERSION", required = true)]
    pub version_pattern: VersionPattern,

    /// Wait for the launched Unity Editor process to exit before this command returns.
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Suppress informational messages from ucom before launching Unity.
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Show the command that would be executed to launch Unity without actually running it.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Command-line arguments to pass directly to the selected Unity Editor executable. Required.
    #[arg(last = true, value_name = "UNITY_ARGS", required = true)]
    pub args: Option<Vec<String>>,
}
