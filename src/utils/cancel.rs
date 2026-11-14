//! Cooperative cancellation with Ctrl+C.
//!
//! Only while ucom waits for Unity (a [`CancellableWait`] is alive) does the Ctrl+C handler
//! count presses instead of exiting. The waiting code checks the count and returns through
//! normal control flow, so destructors run: status lines are cleared, the native progress
//! indicator is hidden, and an injected build script is removed.
//!
//! - First press: cancel. A batch Unity process gets a grace period to exit; it usually received
//!   the same Ctrl+C.
//! - Second press: stop waiting for the grace period and kill the Unity process.
//! - Third press: exit immediately, skipping all cleanup.
//!
//! Anywhere else, for example during a download that cannot be interrupted, the first press
//! exits immediately, as without a handler.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicU32, Ordering};

use crossterm::QueueableCommand;
use crossterm::terminal::{Clear, ClearType};

use crate::utils::{ReportedFailure, warn_non_fatal};

/// The conventional exit code of a process ended by Ctrl+C (128 + SIGINT).
pub const EXIT_CODE: u8 = 130;

/// The number of Ctrl+C presses so far.
static PRESSES: AtomicU32 = AtomicU32::new(0);

/// The number of [`CancellableWait`]s alive.
static WAITS: AtomicU32 = AtomicU32::new(0);

/// Marks a wait that checks [`state`] and returns when cancelled. While one is alive, Ctrl+C
/// requests cancellation instead of ending ucom.
#[must_use = "Ctrl+C is only cooperative while this guard is alive"]
pub struct CancellableWait {
    counter: &'static AtomicU32,
}

impl CancellableWait {
    pub fn start() -> Self {
        Self::counted_in(&WAITS)
    }

    fn counted_in(counter: &'static AtomicU32) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self { counter }
    }
}

impl Drop for CancellableWait {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Whether a Ctrl+C press should end ucom at once: when no wait can handle it, or on the third
/// press. `presses` includes this press.
fn exits_immediately(presses: u32, waits: u32) -> bool {
    waits == 0 || presses >= 3
}

/// The failure that `CommandError::Cancelled` links to as its source, so that `main` finds it.
pub static CANCELLED: ReportedFailure = ReportedFailure::Interrupted;

/// How far cancellation has gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelState {
    /// Not cancelled.
    Running,
    /// Cancelled once: stop, but give a child process time to exit.
    Requested,
    /// Cancelled again: stop now.
    Forced,
}

impl CancelState {
    fn from_presses(presses: u32) -> Self {
        match presses {
            0 => Self::Running,
            1 => Self::Requested,
            _ => Self::Forced,
        }
    }

    pub fn is_cancelled(self) -> bool {
        self != Self::Running
    }
}

/// Installs the Ctrl+C handler. Without it, Ctrl+C ends ucom immediately, as before.
pub fn install_handler() {
    let result = ctrlc::set_handler(|| {
        // Only count here; the waiting code does the rest outside the handler.
        let presses = PRESSES.fetch_add(1, Ordering::SeqCst) + 1;
        if exits_immediately(presses, WAITS.load(Ordering::SeqCst)) {
            // Destructors do not run, so erase a status line such as "Downloading ..." here.
            let mut out = std::io::stdout();
            if out.is_terminal() {
                let _ = clear_line(&mut out);
            }
            exit_interrupted();
        }
    });

    if let Err(e) = result {
        warn_non_fatal!("Ctrl+C cannot be handled, so cancelling skips cleanup: {e}");
    }
}

/// Ends ucom as if Ctrl+C had stopped it.
///
/// On Unix the process ends by SIGINT, not with an exit code. Shells stop a running script or
/// loop only when a command was ended by SIGINT, so `for p in A B; do ucom build ...; done`
/// stops at the first Ctrl+C. Elsewhere it exits with [`EXIT_CODE`].
pub fn exit_interrupted() -> ! {
    let _ = std::io::stdout().flush();
    #[cfg(unix)]
    // SAFETY: resetting SIGINT to its default action and raising it only ends the process.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::raise(libc::SIGINT);
    }
    std::process::exit(i32::from(EXIT_CODE))
}

/// Returns the current cancellation state.
pub fn state() -> CancelState {
    CancelState::from_presses(PRESSES.load(Ordering::SeqCst))
}

/// Returns whether the user has pressed Ctrl+C during a wait.
pub fn is_cancelled() -> bool {
    state().is_cancelled()
}

/// Prints a line to stderr. On a terminal, it replaces a status line that may be shown at the
/// cursor.
pub fn print_notice(message: &str) {
    let mut err = std::io::stderr();
    if err.is_terminal() {
        let _ = clear_line(&mut err);
    }
    let _ = writeln!(err, "{message}").and_then(|()| err.flush());
}

/// Erases the current line from its start, including the `^C` the terminal echoed there.
fn clear_line(out: &mut impl Write) -> std::io::Result<()> {
    out.write_all(b"\r")?;
    out.queue(Clear(ClearType::FromCursorDown))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_map_to_states() {
        assert_eq!(CancelState::from_presses(0), CancelState::Running);
        assert_eq!(CancelState::from_presses(1), CancelState::Requested);
        assert_eq!(CancelState::from_presses(2), CancelState::Forced);
        assert_eq!(CancelState::from_presses(7), CancelState::Forced);
        assert!(!CancelState::Running.is_cancelled());
        assert!(CancelState::Requested.is_cancelled());
    }

    #[test]
    fn ctrl_c_exits_at_once_outside_a_wait_and_on_the_third_press() {
        assert!(exits_immediately(1, 0));
        assert!(!exits_immediately(1, 1));
        assert!(!exits_immediately(2, 1));
        assert!(exits_immediately(3, 1));
    }

    #[test]
    fn cancellable_waits_are_counted_while_alive() {
        // A counter of its own, since other tests start waits on the global one.
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        let outer = CancellableWait::counted_in(&COUNTER);
        let inner = CancellableWait::counted_in(&COUNTER);
        assert_eq!(COUNTER.load(Ordering::SeqCst), 2);
        drop(inner);
        assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
        drop(outer);
        assert_eq!(COUNTER.load(Ordering::SeqCst), 0);
    }
}
