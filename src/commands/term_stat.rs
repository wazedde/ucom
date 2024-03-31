use std::io::{stdout, IsTerminal, Write};

use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::terminal::{Clear, ClearType};
use crossterm::ExecutableCommand;
use strum::{AsRefStr, Display};
use yansi::{Color, Paint, Painted, Style};

/// A status line that is only active if stdout is a terminal.
/// Clears the status line when dropped.
pub(crate) struct TermStat {
    show_output: bool,
}

impl Drop for TermStat {
    fn drop(&mut self) {
        if self.show_output {
            Self::clear_last_line();
        }
    }
}

impl TermStat {
    /// Creates a new `TermStat` message with the given tag and message.
    /// The message is cleared when dropped.
    pub(crate) fn new(tag: &str, msg: &str) -> Self {
        if stdout().is_terminal() {
            Self::print_transient(tag, msg, Status::Info);
            Self { show_output: true }
        } else {
            Self { show_output: false }
        }
    }

    /// Creates a new `TermStat` that does not output anything.
    pub(crate) fn new_null_output() -> Self {
        Self { show_output: false }
    }

    /// Reprints the status line with the given message.
    pub(crate) fn reprint(&self, tag: &str, msg: &str) {
        if self.show_output {
            Self::clear_last_line();
            Self::print_transient(tag, msg, Status::Info);
        };
    }

    /// Prints a status line with the given tag and message.
    pub(crate) fn println(tag: &str, msg: &str, status: Status) {
        println!("{:>12} {}", Self::stylize(tag, status), msg);
    }

    pub(crate) fn stylize(s: &str, status: Status) -> Painted<&str> {
        let color = match status {
            Status::None => Style::new().bold(),
            Status::Ok => Color::Green.bold(),
            Status::Error => Color::Red.bold(),
            Status::Warning => Color::Yellow.bold(),
            Status::Info => Color::Blue.bold(),
        };
        s.paint(color)
    }

    /// Prints a status line with the given tag and message that is cleared.
    fn print_transient(tag: &str, msg: &str, status: Status) {
        _ = stdout().execute(SavePosition).and_then(|o| {
            print!("{:>12} {}", Self::stylize(tag, status), msg);
            o.execute(RestorePosition)?.flush()
        });
    }

    fn clear_last_line() {
        _ = stdout().execute(Clear(ClearType::FromCursorDown));
    }
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
