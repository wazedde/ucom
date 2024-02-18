use std::io::{stdout, IsTerminal, Write};

use colored::Color::Blue;
use colored::{ColoredString, Colorize};
use crossterm::cursor::MoveToColumn;
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};

/// A status line that is only active if stdout is a terminal.
/// Clears the status line when dropped.
pub struct TermStat {
    is_active: bool,
}

impl Drop for TermStat {
    fn drop(&mut self) {
        if self.is_active {
            self.clear_current_line();
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
            Self::print_stat(tag, msg, Status::Info);
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
    pub fn update_text<S1, S2>(&self, tag: S1, msg: S2)
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        if self.is_active {
            self.clear_current_line();
            Self::print_stat(tag, msg, Status::Info);
        }
    }

    fn clear_current_line(&self) {
        _ = execute!(stdout(), Clear(ClearType::CurrentLine), MoveToColumn(0));
    }

    /// Prints a status line with the given tag and message.
    pub fn println_stat<S1, S2>(tag: S1, msg: S2, status: Status)
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        println!(
            "{:>12} {}",
            Self::get_colored_tag(tag, status),
            msg.as_ref()
        );
    }

    /// Prints a status line with the given tag and message.
    pub fn print_stat<S1, S2>(tag: S1, msg: S2, status: Status)
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        print!(
            "{:>12} {}",
            Self::get_colored_tag(tag, status),
            msg.as_ref()
        );
        stdout().flush().unwrap();
    }

    fn get_colored_tag<S1>(tag: S1, status: Status) -> ColoredString
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
#[derive(Debug, Clone, Copy)]
pub enum Status {
    None,
    Ok,
    Error,
    Warning,
    Info,
}
