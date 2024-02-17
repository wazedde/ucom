use std::fmt::Display;
use std::io::{stdout, IsTerminal, Write};

use colored::Colorize;
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
            let ts = Self { is_active: true };
            ts.print_status(tag, msg);
            ts
        } else {
            Self { is_active: false }
        }
    }

    /// Creates a new `TermStat` that is only active if `quiet` is `false`.
    pub fn new_quiet<S1, S2>(quiet: bool, tag: S1, msg: S2) -> Self
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        if quiet {
            Self { is_active: false }
        } else {
            Self::new(tag, msg)
        }
    }

    /// Updates the status line with the given message.
    pub fn update_text<S1, S2>(&self, tag: S1, msg: S2)
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        if self.is_active {
            self.clear_current_line();
            self.print_status(tag, msg);
        }
    }

    fn print_status<S1, S2>(&self, tag: S1, msg: S2)
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        print!("{:>12} {}", tag.as_ref().blue().bold(), msg.as_ref());
        stdout().flush().unwrap();
    }

    fn clear_current_line(&self) {
        _ = execute!(stdout(), Clear(ClearType::CurrentLine));
        _ = execute!(stdout(), MoveToColumn(0));
    }

    /// Prints a status line with the given tag and message when stdout is a terminal.
    pub fn print_stat_ok<S1, S2>(tag: S1, msg: S2)
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        if stdout().is_terminal() {
            Self::print_stat(tag.as_ref().green().bold(), msg.as_ref());
        }
    }

    fn print_stat<S1, S2>(tag: S1, msg: S2)
    where
        S1: Display,
        S2: Display,
    {
        println!("{:>12} {}", tag, msg);
    }
}
