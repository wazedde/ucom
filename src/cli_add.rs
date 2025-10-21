use std::borrow::Cow;
use std::path::PathBuf;

use clap::{Args, ValueEnum};

use crate::utils::content_cache;
use crate::utils::content_cache::RemoteChangeCheck;
use crate::utils::status_line::StatusLine;

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
    #[arg(
        short = 'f',
        long,
        conflicts_with = "display_content",
        conflicts_with = "display_url"
    )]
    pub force: bool,

    /// Print the content of the selected template file to standard output instead of writing it to the project.
    #[arg(
        short = 'c',
        long,
        conflicts_with = "force",
        conflicts_with = "display_url"
    )]
    pub display_content: bool,

    /// Print the source URL of the selected template file to standard output instead of writing it to the project.
    #[arg(
        short = 'u',
        long,
        conflicts_with = "force",
        conflicts_with = "display_content"
    )]
    pub display_url: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum UnityTemplateFile {
    /// Adds 'UnityBuilder.cs', a C# script for automating builds via the command line.
    Builder,
    /// Adds 'EditorMenu.cs', which includes the 'Builder' functionality and adds build commands to the Unity Editor menu.
    BuilderMenu,
    /// Adds a standard '.gitignore' file tailored for Unity projects.
    GitIgnore,
    /// Adds a standard '.gitattributes' file tailored for Unity projects, often used with Git LFS.
    GitAttributes,
}

pub struct TemplateAsset {
    pub filename: &'static str,
    pub content: AssetSource,
}

#[allow(dead_code)]
pub enum AssetSource {
    Static(&'static str),
    Remote(&'static str),
}

impl TemplateAsset {
    pub fn load_content<'a>(&self) -> anyhow::Result<Cow<'a, str>> {
        match self.content {
            AssetSource::Static(content) => Ok(Cow::Borrowed(content)),
            AssetSource::Remote(url) => {
                let _status =
                    StatusLine::new("Downloading", format!("{} from {}...", self.filename, url));
                let content = content_cache::fetch_content(url, RemoteChangeCheck::Skip)?;
                Ok(Cow::Owned(content))
            }
        }
    }
}

impl UnityTemplateFile {
    pub const fn as_asset(self) -> TemplateAsset {
        match self {
            Self::Builder => TemplateAsset {
                filename: "UnityBuilder.cs",
                content: AssetSource::Static(include_str!("../templates/UnityBuilder.cs")),
                // content: AssetSource::Remote(
                //     "https://gist.github.com/jakkovanhunen/b56a70509616b6ff3492a17ae670a5e7/raw",
                // ),
            },
            Self::BuilderMenu => TemplateAsset {
                filename: "EditorMenu.cs",
                content: AssetSource::Static(include_str!("../templates/EditorMenu.cs")),
                // content: AssetSource::Remote(
                //     "https://gist.github.com/jakkovanhunen/a610aa5f675e3826de3b389ddba21319/raw",
                // ),
            },
            Self::GitIgnore => TemplateAsset {
                filename: ".gitignore",
                content: AssetSource::Static(include_str!("../templates/gitignore.txt")),
                // content: AssetSource::Remote(
                //     "https://gist.github.com/jakkovanhunen/5748353142783045c9bc353ed3a341e7/raw",
                // ),
            },
            Self::GitAttributes => TemplateAsset {
                filename: ".gitattributes",
                content: AssetSource::Static(include_str!("../templates/gitattributes.txt")),
                // content: AssetSource::Remote(
                //     "https://gist.github.com/jakkovanhunen/68d2c0e0da4ebfdf9e094b5505c3f337/raw",
                // ),
            },
        }
    }
}
