//! Terminal output shared by the commands: indentation, marks and tags.

use crate::style_definitions::{Styled, Tone};
use std::fmt::{self, Display, Formatter};

pub const INDENT: &str = "  ";

/// A one-character mark in front of a line, in its own colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// No information is available.
    NoInfo,
    /// A plain list item.
    Bullet,
    /// Installed or available.
    Available,
    /// Not installed or not available.
    Unavailable,
    /// A release with a serious issue.
    Error,
    /// A release with a known issue.
    Warning,
}

impl Mark {
    /// Returns [`Available`](Self::Available) or [`Unavailable`](Self::Unavailable).
    pub const fn installed(is_installed: bool) -> Self {
        if is_installed {
            Self::Available
        } else {
            Self::Unavailable
        }
    }
}

impl Display for Mark {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let (symbol, tone) = match self {
            Self::NoInfo => ("?", Tone::Plain),
            Self::Bullet => ("-", Tone::Plain),
            Self::Available => ("✓", Tone::Success),
            Self::Unavailable => ("✗", Tone::Error),
            Self::Error => ("‼", Tone::Error),
            Self::Warning => ("!", Tone::Warning),
        };
        symbol.tone(tone).fmt(f)
    }
}

/// A status tag such as `[OK]`, with the word in its own colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// An update is available.
    Outdated,
    /// A newer version.
    New,
    /// The latest version is installed.
    UpToDate,
}

impl Tag {
    /// Returns the tone of the tag's word, which also suits the version it is shown with.
    pub const fn tone(self) -> Tone {
        match self {
            Self::Outdated => Tone::Outdated,
            Self::New => Tone::Update,
            Self::UpToDate => Tone::Success,
        }
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let word = match self {
            Self::Outdated => "OUTDATED",
            Self::New => "NEW",
            Self::UpToDate => "OK",
        };
        write!(f, "[{}]", word.tone(Self::tone(*self)))
    }
}
