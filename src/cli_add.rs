use std::borrow::Cow;
use std::path::PathBuf;

use clap::{Args, ValueEnum};

use crate::utils::content_cache;
use crate::utils::content_cache::RemoteChangeCheck;
use crate::utils::status_line::StatusLine;

#[derive(Args)]
pub struct AddArguments {
    /// The template file to be added to the project.
    #[arg(value_enum)]
    pub template: UnityTemplateFile,

    /// Defines the project's directory.
    #[arg(
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::DirPath,
        default_value = ".",
        conflicts_with = "display_content"
    )]
    pub project_dir: PathBuf,

    /// Overwrites existing template files.
    #[arg(
        short = 'f',
        long,
        conflicts_with = "display_content",
        conflicts_with = "display_url"
    )]
    pub force: bool,

    /// Displays the template's content to stdout instead of adding it.
    #[arg(
        short = 'c',
        long,
        conflicts_with = "force",
        conflicts_with = "display_url"
    )]
    pub display_content: bool,

    /// Displays the template's source URL.
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
    /// A C# helper script that handles project building.
    Builder,
    /// A C# helper script that adds build commands to Unity's menu (also adds 'builder').
    BuilderMenu,
    /// A Unity specific .gitignore file for newly created projects.
    GitIgnore,
    /// A Unity specific .gitattributes file for newly created projects.
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
                    StatusLine::new("Downloading", &format!("{} from {}...", self.filename, url));
                let content =
                    Cow::Owned(content_cache::fetch_content(url, RemoteChangeCheck::Skip)?);
                Ok(content)
            }
        }
    }
}

impl UnityTemplateFile {
    pub const fn as_asset(self) -> TemplateAsset {
        match self {
            Self::Builder => TemplateAsset {
                filename: "UnityBuilder.cs",
                content: AssetSource::Remote(
                    "https://gist.github.com/jakkovanhunen/b56a70509616b6ff3492a17ae670a5e7/raw",
                ),
            },
            Self::BuilderMenu => TemplateAsset {
                filename: "EditorMenu.cs",
                content: AssetSource::Remote(
                    "https://gist.github.com/jakkovanhunen/a610aa5f675e3826de3b389ddba21319/raw",
                ),
            },
            Self::GitIgnore => TemplateAsset {
                filename: ".gitignore",
                content: AssetSource::Remote(
                    "https://gist.github.com/jakkovanhunen/5748353142783045c9bc353ed3a341e7/raw",
                ),
            },
            Self::GitAttributes => TemplateAsset {
                filename: ".gitattributes",
                content: AssetSource::Remote(
                    "https://gist.github.com/jakkovanhunen/68d2c0e0da4ebfdf9e094b5505c3f337/raw",
                ),
            },
        }
    }
}
