use std::fmt::{Display, Formatter};
use std::io::{stdout, IsTerminal, Write};

use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::terminal::{Clear, ClearType};
use crossterm::ExecutableCommand;
use yansi::{Color, Paint, Painted};

/// A status line that is only active if stdout is a terminal.
/// Clears the status line when dropped.
pub struct TermStat {
    is_active: bool,
}

impl Drop for TermStat {
    fn drop(&mut self) {
        if self.is_active {
            _ = Self::clear_last_line();
        }
    }
}

impl TermStat {
    /// Creates a new `TermStat`.
    pub fn new(tag: &str, msg: &str) -> Self {
        if stdout().is_terminal() {
            _ = Self::print_stat(tag, msg, Status::Info);
            Self { is_active: true }
        } else {
            Self { is_active: false }
        }
    }

    /// Creates a new `TermStat` that is inactive.
    pub fn new_inactive() -> Self {
        Self { is_active: false }
    }

    /// Updates the status line with the given message.
    pub fn update_text(&self, tag: &str, msg: &str) -> anyhow::Result<()> {
        if self.is_active {
            Self::clear_last_line()?;
            Self::print_stat(tag, msg, Status::Info)?;
        };
        Ok(())
    }

    pub fn clear_last_line() -> anyhow::Result<()> {
        stdout().execute(Clear(ClearType::FromCursorDown))?;
        Ok(())
    }

    /// Prints a status line with the given tag and message.
    pub fn println_stat(tag: &str, msg: &str, status: Status) {
        println!("{:>12} {}", Self::colorize(tag, status), msg);
    }

    /// Prints a status line with the given tag and message
    /// and moves the cursor back to the start of the line.
    pub fn print_stat(tag: &str, msg: &str, status: Status) -> anyhow::Result<()> {
        stdout().execute(SavePosition)?;

        print!("{:>12} {}", Self::colorize(tag, status), msg);
        stdout().execute(RestorePosition)?.flush()?;
        Ok(())
    }

    pub fn colorize(s: &str, status: Status) -> Painted<&str> {
        let color = match status {
            Status::None => Color::Primary,
            Status::Ok => Color::Green,
            Status::Error => Color::Red,
            Status::Warning => Color::Yellow,
            Status::Info => Color::Blue,
        };
        s.fg(color).bold()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    None,
    Ok,
    Error,
    Warning,
    Info,
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Status::None => "None",
            Status::Ok => "Ok",
            Status::Error => "Error",
            Status::Warning => "Warning",
            Status::Info => "Info",
        };
        write!(f, "{}", str)
    }
}
