use std::fmt::Display;
use std::io::{self, IsTerminal, Write, stdout};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::style_definitions::{Styled, Tone};
use crossterm::QueueableCommand;
use crossterm::terminal::{Clear, ClearType};

/// Width of the right-aligned label of status lines and status messages, as cargo prints them.
const LABEL_PADDING: usize = 12;

/// A temporary progress line on stdout, such as "Downloading ...", that is cleared when dropped.
/// It is only shown when stdout is a terminal.
///
/// The line is cut to the terminal width, so it never wraps, and the cursor is left at its
/// start: other output then starts on that line. Drop the status line before printing to
/// stdout; print to stderr with [`eprint_line`].
///
/// Status lines can be nested. Only the newest is shown; when it is dropped, the one before it
/// is shown again.
pub struct StatusLine {
    /// The line's entry in [`SHOWN`], or `None` if nothing is shown.
    id: Option<u64>,
}

/// The status lines that are alive, oldest first, and whether the newest is on screen.
struct Shown {
    lines: Vec<Line>,
    visible: bool,
}

struct Line {
    id: u64,
    label: &'static str,
    msg: String,
}

static SHOWN: Mutex<Shown> = Mutex::new(Shown {
    lines: Vec::new(),
    visible: false,
});

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn shown() -> MutexGuard<'static, Shown> {
    SHOWN.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shown {
    /// Draws the newest status line, if any.
    fn redraw(&mut self) {
        if let Some(line) = self.lines.last() {
            let width = crossterm::terminal::size().map_or(80, |(w, _)| usize::from(w));
            draw(line.label, &fit_message(line.label, &line.msg, width)).ok();
            self.visible = true;
        }
    }

    /// Runs `f` with the status line off the screen, then shows the newest line again if one
    /// was shown.
    fn hidden_while(&mut self, f: impl FnOnce(&mut Self)) {
        let was_visible = self.visible;
        self.hide();
        f(self);
        if was_visible {
            self.redraw();
        }
    }

    /// Clears the status line from the screen; it stays alive.
    fn hide(&mut self) {
        if self.visible {
            self.visible = false;
            let mut out = stdout().lock();
            clear_line(&mut out).and_then(|()| out.flush()).ok();
        }
    }
}

impl StatusLine {
    /// Shows `label` and `msg` if stdout is a terminal.
    pub fn new(label: &'static str, msg: impl Display) -> Self {
        Self::new_if(true, label, msg)
    }

    /// Shows `label` and `msg` as [`StatusLine::new`] does if `show` is true; otherwise
    /// shows nothing.
    pub fn new_if(show: bool, label: &'static str, msg: impl Display) -> Self {
        if !(show && stdout().is_terminal()) {
            return Self { id: None };
        }
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut shown = shown();
        shown.lines.push(Line {
            id,
            label,
            msg: msg.to_string(),
        });
        shown.redraw();
        Self { id: Some(id) }
    }

    /// Replaces the message.
    pub fn update(&self, msg: impl Display) {
        let Some(id) = self.id else { return };
        let mut shown = shown();
        let Some(index) = shown.lines.iter().position(|line| line.id == id) else {
            return;
        };
        shown.lines[index].msg = msg.to_string();
        if index + 1 == shown.lines.len() {
            shown.redraw();
        }
    }
}

impl Drop for StatusLine {
    fn drop(&mut self) {
        if let Some(id) = self.id {
            shown().hidden_while(|shown| shown.lines.retain(|line| line.id != id));
        }
    }
}

/// Draws the line over the current one, clears what is left of it, and leaves the cursor at
/// its start.
fn draw(label: &str, msg: &str) -> io::Result<()> {
    let mut out = stdout().lock();
    write!(
        out,
        "\r{:>LABEL_PADDING$} {msg}",
        label.tone(Tone::Info).bold()
    )?;
    out.queue(Clear(ClearType::UntilNewLine))?;
    out.write_all(b"\r")?;
    out.flush()
}

/// Cuts `msg` so that the line with `label` fits in `width` columns. The middle is replaced
/// by `…`, so the start and the end stay visible, such as a percentage or a project name. The
/// last column stays empty, because some terminals wrap when it is written. Counts characters,
/// not display width.
fn fit_message(label: &str, msg: &str, width: usize) -> String {
    let used = label.chars().count().max(LABEL_PADDING) + 1;
    let room = width.saturating_sub(used + 1);
    let len = msg.chars().count();
    if len <= room {
        return msg.to_owned();
    }
    let Some(keep) = room.checked_sub(1) else {
        return String::new();
    };
    let tail = keep / 2;
    let head = keep - tail;
    let mut cut: String = msg.chars().take(head).collect();
    cut.push('…');
    cut.extend(msg.chars().skip(len - tail));
    cut
}

/// Erases the current line from its start, and anything below it. Does not flush.
pub fn clear_line(out: &mut impl Write) -> io::Result<()> {
    out.write_all(b"\r")?;
    out.queue(Clear(ClearType::FromCursorDown))?;
    Ok(())
}

/// Prints a line to stderr. A status line that is shown is cleared first and shown again
/// afterwards, so that the two do not mix.
pub fn eprint_line(msg: impl Display) {
    shown().hidden_while(|_| eprintln!("{msg}"));
}

//
// Status messages
//

/// Prints a status message, as cargo does: `label` in bold in the colour of `tone`, right-aligned,
/// then `msg`.
pub fn print_status(tone: Tone, label: &str, msg: impl Display) {
    println!("{:>LABEL_PADDING$} {msg}", label.tone(tone).bold());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_that_fits_is_kept() {
        assert_eq!(
            fit_message("Downloading", "release data", 80),
            "release data"
        );
    }

    #[test]
    fn a_long_message_is_cut_to_the_terminal_width() {
        // 12 columns of label, a space, the message, and the empty last column.
        let msg = fit_message("Downloading", "0123456789", 20);
        assert_eq!(msg, "012…89");
        assert_eq!(LABEL_PADDING + 1 + msg.chars().count() + 1, 20);
    }

    #[test]
    fn a_long_label_leaves_less_room() {
        assert_eq!(fit_message("Building via editor", "0123456789", 25), "01…9");
    }

    #[test]
    fn no_room_gives_an_empty_message() {
        assert_eq!(fit_message("Downloading", "0123456789", 10), "");
    }

    #[test]
    fn a_download_keeps_its_percentage() {
        let msg = fit_message("Downloading", "Full Unity release data... (4%)", 40);
        assert!(msg.ends_with("(4%)"), "{msg}");
        assert!(msg.starts_with("Full Unity"), "{msg}");
    }
}
