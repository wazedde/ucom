//! Template files that ucom adds to Unity projects, embedded in the binary.

use clap::ValueEnum;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum UnityTemplateFile {
    /// Adds 'UnityBuilder.cs', a C# script for automating builds via the command line and enabling editor IPC.
    Builder,
    /// Adds 'EditorMenu.cs', which includes the 'Builder' functionality and adds build commands to the Unity Editor menu.
    BuilderMenu,
    /// Adds a standard '.gitignore' file tailored for Unity projects.
    GitIgnore,
    /// Adds a standard '.gitattributes' file tailored for Unity projects, often used with Git LFS.
    GitAttributes,
}

/// A template file: its file name in the project and its embedded content.
pub struct TemplateAsset {
    pub filename: &'static str,
    pub content: &'static str,
}

impl UnityTemplateFile {
    pub const fn as_asset(self) -> TemplateAsset {
        match self {
            Self::Builder => TemplateAsset {
                filename: "UnityBuilder.cs",
                content: include_str!("../templates/UnityBuilder.cs"),
            },
            Self::BuilderMenu => TemplateAsset {
                filename: "EditorMenu.cs",
                content: include_str!("../templates/EditorMenu.cs"),
            },
            Self::GitIgnore => TemplateAsset {
                filename: ".gitignore",
                content: include_str!("../templates/gitignore.txt"),
            },
            Self::GitAttributes => TemplateAsset {
                filename: ".gitattributes",
                content: include_str!("../templates/gitattributes.txt"),
            },
        }
    }
}
