use std::io;
use std::io::{IsTerminal, Write, stdout};

use crate::style_definitions::{Styled, Tone};
use crossterm::ExecutableCommand;
use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::terminal::{Clear, ClearType};
use std::fmt::Display;

//
// Status line
//

const LABEL_PADDING: usize = 12;

/// A status line that is only active if stdout is a terminal.
/// Clears the status line when dropped.
pub enum StatusLine {
    Silent,
    Stdout(StdoutStatusLine),
}

impl StatusLine {
    /// Creates a new `StatusLine` message with the given label and message that is cleared when dropped.
    /// If stdout is not a terminal, it will not output anything.
    pub fn new(label: impl AsRef<str>, msg: impl AsRef<str>) -> Self {
        if stdout().is_terminal() {
            Self::Stdout(StdoutStatusLine::new(label, msg))
        } else {
            Self::Silent
        }
    }

    /// Creates a `StatusLine` as [`StatusLine::new`] does if `show` is true, otherwise a silent one.
    pub fn new_if(show: bool, label: impl AsRef<str>, msg: impl AsRef<str>) -> Self {
        if show {
            Self::new(label, msg)
        } else {
            Self::Silent
        }
    }

    /// Updates the status line with the given label and message.
    pub fn update_line(&self, label: impl AsRef<str>, msg: impl AsRef<str>) {
        match self {
            Self::Stdout(..) => {
                StdoutStatusLine::update_line(label, msg);
            }
            Self::Silent => { /* do nothing */ }
        }
    }
}

//
// Stdout status line
//

/// Terminal status line implementation that is cleared on drop
pub struct StdoutStatusLine {}

impl Drop for StdoutStatusLine {
    fn drop(&mut self) {
        Self::clear_last_line().ok();
    }
}

impl StdoutStatusLine {
    pub fn new(label: impl AsRef<str>, msg: impl AsRef<str>) -> Self {
        let status_line = Self {};
        Self::print_line(label, msg).ok();
        status_line
    }

    pub fn update_line(label: impl AsRef<str>, msg: impl AsRef<str>) {
        Self::clear_last_line().ok();
        Self::print_line(label, msg).ok();
    }

    fn print_line(label: impl AsRef<str>, msg: impl AsRef<str>) -> io::Result<()> {
        stdout().execute(SavePosition).and_then(|o| {
            print!(
                "{:>LABEL_PADDING$} {}",
                label.as_ref().tone(Tone::Info).bold(),
                msg.as_ref()
            );
            o.execute(RestorePosition)?.flush()
        })
    }

    fn clear_last_line() -> io::Result<()> {
        stdout()
            .execute(Clear(ClearType::FromCursorDown))
            .map(|_| ())
    }
}

//
// Status messages
//

/// Prints a status message, as cargo does: `label` in bold in the colour of `tone`, right-aligned,
/// then `msg`.
pub fn print_status(tone: Tone, label: &str, msg: impl Display) {
    println!("{:>LABEL_PADDING$} {msg}", label.tone(tone).bold());
}
