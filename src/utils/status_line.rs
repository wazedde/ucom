use std::io;
use std::io::{IsTerminal, Write, stdout};

use crossterm::ExecutableCommand;
use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::terminal::{Clear, ClearType};
use strum::{AsRefStr, Display};
use yansi::{Color, Paint, Painted, Style};

//
// Status line
//

/// A status line that is only active if stdout is a terminal.
/// Clears the status line when dropped.
pub enum StatusLine {
    Silent,
    Stdout(StdoutStatusLine),
}

impl StatusLine {
    /// Creates a new `StatusLine` message with the given label and message that is cleared when dropped.
    /// If stdout is not a terminal, it will not output anything.
    pub fn new(label: &str, msg: &str) -> Self {
        if stdout().is_terminal() {
            Self::Stdout(StdoutStatusLine::new(label, msg, MessageType::Info))
        } else {
            Self::Silent
        }
    }

    /// Creates a new `StatusLine` that does not output anything.
    pub const fn new_silent() -> Self {
        Self::Silent
    }

    /// Updates the status line with the given label and message.
    pub fn update_line(&self, label: &str, msg: &str) {
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
    pub fn new(label: &str, msg: &str, message_type: MessageType) -> Self {
        let status_line = Self {};
        Self::print_line(label, msg, message_type).ok();
        status_line
    }

    pub fn update_line(label: &str, msg: &str) {
        Self::clear_last_line().ok();
        Self::print_line(label, msg, MessageType::Info).ok();
    }

    fn print_line(label: &str, msg: &str, status: MessageType) -> io::Result<()> {
        const LABEL_PADDING: usize = 12;
        stdout().execute(SavePosition).and_then(|o| {
            print!(
                "{:>LABEL_PADDING$} {}",
                MessageType::format_text(label, status),
                msg
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
// Message type
//

/// The type of message to display.
#[allow(dead_code)]
#[derive(Display, AsRefStr, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Plain,
    Ok,
    Error,
    Warning,
    Info,
}

impl MessageType {
    /// Applies a style to the given string based on the message type.
    pub fn format_text(s: &str, message_type: Self) -> Painted<&str> {
        let color = match message_type {
            Self::Plain => Style::new().bold(),
            Self::Ok => Color::Green.bold(),
            Self::Error => Color::Red.bold(),
            Self::Warning => Color::Yellow.bold(),
            Self::Info => Color::Blue.bold(),
        };
        s.paint(color)
    }

    /// Prints a status line with the given label and message.
    pub fn print_line(label: &str, msg: &str, message_type: Self) {
        println!("{:>12} {}", Self::format_text(label, message_type), msg);
    }
}
