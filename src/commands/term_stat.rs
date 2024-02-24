use std::fmt::{Display, Formatter};
use std::io::{stdout, IsTerminal, Write};

use colored::Color::Blue;
use colored::{ColoredString, Colorize};
use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::terminal::{Clear, ClearType};
use crossterm::ExecutableCommand;

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
    pub fn new<S1, S2>(tag: S1, msg: S2) -> Self
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
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
    pub fn update_text<S1, S2>(&self, tag: S1, msg: S2) -> anyhow::Result<()>
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
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
    pub fn println_stat<S1, S2>(tag: S1, msg: S2, status: Status)
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        println!("{:>12} {}", Self::get_colored(tag, status), msg.as_ref());
    }

    /// Prints a status line with the given tag and message
    /// and moves the cursor back to the start of the line.
    pub fn print_stat<S1, S2>(tag: S1, msg: S2, status: Status) -> anyhow::Result<()>
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        stdout().execute(SavePosition)?;

        print!("{:>12} {}", Self::get_colored(tag, status), msg.as_ref());

        stdout().execute(RestorePosition)?.flush()?;
        Ok(())
    }

    pub fn get_colored<S1>(tag: S1, status: Status) -> ColoredString
    where
        S1: AsRef<str>,
    {
        match status {
            Status::None => tag.as_ref().bold(),
            Status::Ok => tag.as_ref().green().bold(),
            Status::Error => tag.as_ref().red().bold(),
            Status::Warning => tag.as_ref().yellow().bold(),
            Status::Info => tag.as_ref().color(Blue).bold(),
        }
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
