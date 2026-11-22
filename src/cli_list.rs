use clap::{Args, ValueEnum};
use strum::Display;

use crate::unity::VersionPattern;

#[derive(Args)]
pub struct ListArguments {
    /// Specify the type of versions to list.
    #[arg(value_enum, default_value = "installed")]
    pub list_type: ListType,

    /// Filter versions by a version or pattern (e.g., '2022.3', '2022.3.5f1', '2022.3.*', '202*').
    ///
    /// Each given component must match as a whole number, so '2022.3.1' does not match
    /// '2022.3.10f1'. Use '*' for wildcards.
    #[arg(short = 'u', long = "unity", value_name = "VERSION")]
    pub version_filter: Option<VersionPattern>,

    /// Force download of release data from the Unity API, bypassing cache.
    #[arg(short = 'f', long)]
    pub force: bool,
}

#[derive(Debug, Display, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ListType {
    /// List Unity versions currently installed on the system.
    Installed,
    /// List installed versions and check for available updates for each.
    Updates,
    /// List the latest available version for each major/minor release series.
    Latest,
    /// List all known available Unity versions from the release data.
    All,
}
