use std::borrow::Cow;
use std::path::PathBuf;

use clap::{Args, ValueEnum};

use crate::commands::term_stat::TermStat;
use crate::unity::http_cache;

#[derive(Args)]
pub struct AddArguments {
    /// The file to be added to the project.
    #[arg(value_enum)]
    pub file: IncludedFile,

    /// Defines the project's directory.
    #[arg(
        value_name = "DIRECTORY",
        value_hint = clap::ValueHint::DirPath,
        default_value = ".",
        conflicts_with = "display_content"
    )]
    pub project_dir: PathBuf,

    /// Overwrites existing files.
    #[arg(
        short = 'f',
        long,
        conflicts_with = "display_content",
        conflicts_with = "display_url"
    )]
    pub force: bool,

    /// Displays the file's content to stdout instead of adding it.
    #[arg(
        short = 'c',
        long,
        conflicts_with = "force",
        conflicts_with = "display_url"
    )]
    pub display_content: bool,

    /// Displays the file's source URL.
    #[arg(
        short = 'u',
        long,
        conflicts_with = "force",
        conflicts_with = "display_content"
    )]
    pub display_url: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum IncludedFile {
    /// A C# helper script that handles project building.
    Builder,
    /// A C# helper script that adds build commands to Unity's menu (also adds 'builder').
    BuilderMenu,
    /// A Unity specific .gitignore file for newly created projects.
    GitIgnore,
    /// A Unity specific .gitattributes file for newly created projects.
    GitAttributes,
}

pub struct FileData {
    pub filename: &'static str,
    pub content: ContentType,
}

#[allow(dead_code)]
pub enum ContentType {
    Included(&'static str),
    Url(&'static str),
}

impl FileData {
    pub fn fetch_content<'a>(&self) -> anyhow::Result<Cow<'a, str>> {
        return match self.content {
            ContentType::Included(content) => Ok(Cow::Borrowed(content)),
            ContentType::Url(url) => {
                _ = TermStat::new("Downloading", format!("{} from {}...", self.filename, url));
                Ok(Cow::Owned(http_cache::fetch_content(url)?))
            }
        };
    }
}

impl IncludedFile {
    pub const fn data(self) -> FileData {
        match self {
            Self::Builder => FileData {
                filename: "UnityBuilder.cs",
                content: ContentType::Url(
                    "https://gist.github.com/jakkovanhunen/b56a70509616b6ff3492a17ae670a5e7/raw",
                ),
            },
            Self::BuilderMenu => FileData {
                filename: "EditorMenu.cs",
                content: ContentType::Url(
                    "https://gist.github.com/jakkovanhunen/a610aa5f675e3826de3b389ddba21319/raw",
                ),
            },
            Self::GitIgnore => FileData {
                filename: ".gitignore",
                content: ContentType::Url(
                    "https://gist.github.com/jakkovanhunen/5748353142783045c9bc353ed3a341e7/raw",
                ),
            },
            Self::GitAttributes => FileData {
                filename: ".gitattributes",
                content: ContentType::Url(
                    "https://gist.github.com/jakkovanhunen/68d2c0e0da4ebfdf9e094b5505c3f337/raw",
                ),
            },
        }
    }
}
