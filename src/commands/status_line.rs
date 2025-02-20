use std::io::{IsTerminal, Write, stdout};

use crossterm::ExecutableCommand;
use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::terminal::{Clear, ClearType};
use strum::{AsRefStr, Display};
use yansi::{Color, Paint, Painted, Style};

/// A status line that is only active if stdout is a terminal.
/// Clears the status line when dropped.
pub(crate) struct StatusLine {
    show_output: bool,
}

impl Drop for StatusLine {
    fn drop(&mut self) {
        if self.show_output {
            Self::clear_last_line();
        }
    }
}

impl StatusLine {
    /// Creates a new `StatusLine` message with the given tag and message.
    /// The message is cleared when dropped.
    pub(crate) fn new(tag: &str, msg: &str) -> Self {
        let show_output = stdout().is_terminal();
        if show_output {
            Self::print_transient(tag, msg, Status::Info);
        }
        Self { show_output }
    }

    /// Creates a new `StatusLine` that does not output anything.
    pub(crate) fn new_silent() -> Self {
        Self { show_output: false }
    }

    /// Reprints the status line with the given message.
    pub(crate) fn update(&self, tag: &str, msg: &str) {
        if self.show_output {
            Self::clear_last_line();
            Self::print_transient(tag, msg, Status::Info);
        }
    }

    /// Prints a status line with the given tag and message that is cleared.
    fn print_transient(tag: &str, msg: &str, status: Status) {
        _ = stdout().execute(SavePosition).and_then(|o| {
            print!("{:>12} {}", apply_status_style(tag, status), msg);
            o.execute(RestorePosition)?.flush()
        });
    }

    fn clear_last_line() {
        _ = stdout().execute(Clear(ClearType::FromCursorDown));
    }
}

/// Prints a status line with the given tag and message.
pub(crate) fn print_status(tag: &str, msg: &str, status: Status) {
    println!("{:>12} {}", apply_status_style(tag, status), msg);
}

/// Applies a style to the given string based on the status.
pub(crate) fn apply_status_style(s: &str, status: Status) -> Painted<&str> {
    let color = match status {
        Status::None => Style::new().bold(),
        Status::Ok => Color::Green.bold(),
        Status::Error => Color::Red.bold(),
        Status::Warning => Color::Yellow.bold(),
        Status::Info => Color::Blue.bold(),
    };
    s.paint(color)
}

#[allow(dead_code)]
#[derive(Display, AsRefStr, Debug, Clone, Copy, PartialEq)]
pub(crate) enum Status {
    None,
    Ok,
    Error,
    Warning,
    Info,
}
