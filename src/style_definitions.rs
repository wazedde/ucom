//! Terminal styles. Code says what a piece of text means (a [`Tone`]); the tone decides how it
//! looks.

use std::fmt::Display;
use yansi::Color::{Blue, BrightBlack, BrightBlue, Cyan, Green, Red, Yellow};
use yansi::{Paint, Painted, Style};

/// What a piece of text means, which decides its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Text without a colour.
    Plain,
    /// Success, an installed or up-to-date version.
    Success,
    /// Errors, failures and versions that are not installed.
    Error,
    /// Warnings and non-critical issues.
    Warning,
    /// Progress and other information.
    Info,
    /// Clickable links and URLs.
    Link,
    /// A version that has an update available.
    Outdated,
    /// A newer version that can be installed.
    Update,
    /// Text that matters little, such as missing update information.
    Muted,
}

impl Tone {
    /// Returns the style for text with this tone.
    pub const fn style(self) -> Style {
        match self {
            Self::Plain => Style::new(),
            Self::Success => Green.foreground(),
            Self::Error => Red.foreground(),
            Self::Warning => Yellow.foreground(),
            Self::Info | Self::Update => Blue.foreground(),
            Self::Link => BrightBlue.underline(),
            Self::Outdated => Cyan.foreground(),
            Self::Muted => BrightBlack.foreground(),
        }
    }
}

/// Styling for anything that can be displayed.
pub trait Styled: Display {
    /// Shows the text in the style of `tone`.
    fn tone(&self, tone: Tone) -> Painted<&Self> {
        self.paint(tone.style())
    }

    /// Shows all of the text in bold, also after the parts of it that are already styled.
    fn bold_all(&self) -> Painted<&Self> {
        self.bold().wrap()
    }

    /// Like [`bold_all`](Self::bold_all) if `on` is true; otherwise shows the text unchanged.
    fn bold_if(&self, on: bool) -> Painted<&Self> {
        if on {
            self.bold_all()
        } else {
            self.paint(Style::new())
        }
    }
}

impl<T: Display + ?Sized> Styled for T {}
