use std::io::{stdout, IsTerminal, Write};

use crossterm::cursor::{RestorePosition, SavePosition};
use crossterm::terminal::{Clear, ClearType};
use crossterm::ExecutableCommand;
use strum::{AsRefStr, Display};
use yansi::{Color, Paint, Painted, Style};

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
    /// Creates a new `TermStat` message with the given tag and message.
    /// The message is cleared when dropped.
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
    pub fn println(tag: &str, msg: &str, status: Status) {
        println!("{:>12} {}", Self::stylize(tag, status), msg);
    }

    /// Prints a status line with the given tag and message
    /// and moves the cursor back to the start of the line.
    pub fn print_stat(tag: &str, msg: &str, status: Status) -> anyhow::Result<()> {
        stdout().execute(SavePosition)?;

        print!("{:>12} {}", Self::stylize(tag, status), msg);
        stdout().execute(RestorePosition)?.flush()?;
        Ok(())
    }

    pub fn stylize(s: &str, status: Status) -> Painted<&str> {
        let color = match status {
            Status::None => Style::new().bold(),
            Status::Ok => Color::Green.bold(),
            Status::Error => Color::Red.bold(),
            Status::Warning => Color::Yellow.bold(),
            Status::Info => Color::Blue.bold(),
        };
        s.paint(color)
    }
}

#[allow(dead_code)]
#[derive(Display, AsRefStr, Debug, Clone, Copy, PartialEq)]
pub enum Status {
    None,
    Ok,
    Error,
    Warning,
    Info,
}
