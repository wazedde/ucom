use std::borrow::Cow;
use std::io::IsTerminal;

use spinoff::{spinners, Spinner};

/// Terminal spinner that shows a spinner when output is to a terminal.
/// Clears the spinner line when dropped.
pub struct TerminalSpinner {
    spinner: Option<Spinner>,
}

impl Drop for TerminalSpinner {
    fn drop(&mut self) {
        if let Some(mut spinner) = self.spinner.take() {
            spinner.clear();
        }
    }
}

#[allow(dead_code)]
impl TerminalSpinner {
    pub fn new<S>(msg: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        Self {
            spinner: if std::io::stdout().is_terminal() {
                Some(Spinner::new(spinners::Dots, msg, None))
            } else {
                None
            },
        }
    }

    pub fn clear(mut self) {
        if let Some(mut spinner) = self.spinner.take() {
            spinner.clear();
        }
    }

    pub fn update_text<S>(&mut self, msg: S)
    where
        S: Into<Cow<'static, str>>,
    {
        if let Some(spinner) = self.spinner.as_mut() {
            spinner.update_text(msg);
        }
    }
}
