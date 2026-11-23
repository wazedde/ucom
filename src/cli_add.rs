use std::path::PathBuf;

use clap::Args;

use crate::templates::UnityTemplateFile;

#[derive(Args)]
pub struct AddArguments {
    /// Select the helper script or configuration file template to add.
    #[arg(value_enum)]
    pub template: UnityTemplateFile,

    /// Path to the Unity project directory where the file should be added. Defaults to the current directory.
    #[arg(
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::DirPath,
        default_value = ".",
        conflicts_with = "display_content"
    )]
    pub project_dir: PathBuf,

    /// Overwrite the target file if it already exists in the project directory.
    /// For builder-menu, this also overwrites the builder script it requires.
    #[arg(short = 'f', long, conflicts_with = "display_content")]
    pub force: bool,

    /// Print the content of the selected template file to standard output instead of writing it to the project.
    #[arg(short = 'c', long)]
    pub display_content: bool,
}
