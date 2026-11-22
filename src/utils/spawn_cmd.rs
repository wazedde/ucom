use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::time::Instant;
use std::{fs, io, thread};

use crate::utils::cancel::{self, CancelState, CancellableWait};
use crate::utils::warn_non_fatal;

//
// Error implementation
//

/// Errors that can occur when working with a [`Command`].
#[derive(Debug)]
pub enum CommandError {
    /// The command could not be started, or waiting for it failed.
    Io(io::Error),
    /// The command ran and did not succeed. `code` is `None` when it was ended by a signal.
    Failed { code: Option<i32>, stderr: String },
    /// The user cancelled while waiting for the command.
    Cancelled,
}

impl CommandError {
    /// The exit code of a command that ran and failed. `None` if it could not be started, or
    /// was ended by a signal.
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Failed { code, .. } => *code,
            Self::Io(_) | Self::Cancelled => None,
        }
    }
}

impl Error for CommandError {
    // No source for `Io`: the message already includes the I/O error, which would otherwise be
    // printed twice in an error chain. `Cancelled` links to the failure that `main` looks for.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cancelled => Some(&cancel::CANCELLED),
            Self::Io(_) | Self::Failed { .. } => None,
        }
    }
}

impl From<io::Error> for CommandError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl Display for CommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Failed to run the command: {e}"),
            Self::Failed {
                code: Some(code),
                stderr,
            } => write!(f, "Command failed with exit code: {code} {stderr}"),
            Self::Failed { code: None, stderr } => {
                write!(f, "Command was terminated by a signal {stderr}")
            }
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

//
// Command spawning
//

/// Returns the full command line string.
pub fn build_command_line(cmd: &Command) -> String {
    let mut line = cmd.get_program().to_string_lossy().to_string();

    // Handle spaces in the path.
    if line.contains(char::is_whitespace) {
        if cfg!(target_os = "windows") {
            line = format!("& \"{line}\"");
        } else {
            line = format!("\"{line}\"");
        }
    }

    for arg in cmd.get_args() {
        line.push(' ');
        let arg = arg.to_string_lossy();
        // Handle spaces in arguments.
        if arg.contains(char::is_whitespace) {
            line.push('\"');
            line.push_str(&arg);
            line.push('\"');
        } else {
            line.push_str(&arg);
        }
    }
    line
}

/// What a wait does with the child process when the user cancels with Ctrl+C.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnCancel {
    /// Give the child a grace period to exit, then kill it. For batch processes such as builds
    /// and test runs.
    Kill,
    /// Stop waiting and leave the child running. For an interactive editor, which may have
    /// unsaved work. (On macOS and Linux, the editor receives the same Ctrl+C itself.)
    Detach,
}

/// How often a wait checks whether the child has exited or the user cancelled.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How long a cancelled batch process may take to exit before it is killed.
const KILL_GRACE_PERIOD: Duration = Duration::from_secs(10);

/// Spawns command and outputs Unity's log to the console. Blocks until the command has finished
/// or the user cancels; a cancelled command is stopped as [`OnCancel::Kill`] describes.
pub fn wait_with_log_output(mut cmd: Command, log_file: &Path) -> Result<(), CommandError> {
    let _cancellable = CancellableWait::start();
    ensure_not_cancelled()?;
    // Unity writes its log to `log_file`. Stderr is kept for the error message, and read on a
    // thread so that a full pipe never blocks Unity.
    let mut child = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).spawn()?;
    let stderr_reader = child.stderr.take().map(|mut pipe| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            String::from_utf8_lossy(&bytes).into_owned()
        })
    });

    let stop_monitoring = Arc::new(AtomicBool::new(false));

    let log_monitor = thread::spawn({
        let stop_monitoring = stop_monitoring.clone();
        let log_file = log_file.to_owned();
        move || monitor_log_file(&log_file, Duration::from_millis(100), &stop_monitoring)
    });

    let status = wait_for_child(&mut child, OnCancel::Kill, KILL_GRACE_PERIOD, cancel::state);
    stop_monitoring.store(true, Ordering::Release);

    // Wait for the log monitor thread to finish.
    let monitored = log_monitor
        .join()
        .map_err(|e| io::Error::other(format!("Echo runner thread panicked: {e:?}")));

    let status = status?;
    // The log is only shown; Unity's exit status decides the outcome.
    if let Err(e) = monitored.flatten() {
        warn_non_fatal!("Stopped showing the Unity log: {e}");
    }
    // Processes started by Unity can keep the stderr pipe open after Unity exits, so this wait
    // can be cancelled too.
    let stderr = match stderr_reader {
        Some(reader) => {
            while !reader.is_finished() {
                ensure_not_cancelled()?;
                thread::sleep(POLL_INTERVAL);
            }
            reader.join().unwrap_or_default()
        }
        None => String::new(),
    };
    check_exit_status(status, stderr)
}

/// Spawns command and immediately returns without any output.
///
/// Output is discarded via the null device rather than unread pipes, so the
/// child never writes into a closed pipe after the handles are dropped.
pub fn spawn_and_forget(mut cmd: Command) -> Result<(), CommandError> {
    cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    Ok(())
}

/// Spawns command and outputs to the console. Blocks until the command has finished or the user
/// cancels; `on_cancel` decides what happens to the command then.
pub fn wait_with_stdout(mut cmd: Command, on_cancel: OnCancel) -> Result<(), CommandError> {
    let _cancellable = CancellableWait::start();
    ensure_not_cancelled()?;
    let mut child = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let status = wait_for_child(&mut child, on_cancel, KILL_GRACE_PERIOD, cancel::state)?;
    // Stderr went to the console, so there is nothing to add to the error.
    check_exit_status(status, String::new())
}

/// Returns [`CommandError::Cancelled`] if the user has already cancelled, for example while the
/// caller prepared the command, so that no process is started.
fn ensure_not_cancelled() -> Result<(), CommandError> {
    if cancel::is_cancelled() {
        Err(CommandError::Cancelled)
    } else {
        Ok(())
    }
}

/// Returns an error if the process did not exit successfully.
fn check_exit_status(status: ExitStatus, stderr: String) -> Result<(), CommandError> {
    if status.success() {
        Ok(())
    } else {
        Err(CommandError::Failed {
            code: status.code(),
            stderr,
        })
    }
}

/// Waits until `child` exits, or `cancel_state` reports that the user cancelled.
///
/// On cancellation, returns [`CommandError::Cancelled`]. With [`OnCancel::Kill`], the child first
/// gets `grace` to exit, then is killed; a forced cancel (a second Ctrl+C) kills it at once.
fn wait_for_child(
    child: &mut Child,
    on_cancel: OnCancel,
    grace: Duration,
    cancel_state: impl Fn() -> CancelState,
) -> Result<ExitStatus, CommandError> {
    loop {
        if let Some(status) = child.try_wait()? {
            // A terminal sends Ctrl+C to the child too, which then often exits on its own
            // before this loop sees the cancellation. That still counts as cancelled.
            return if cancel_state().is_cancelled() {
                Err(CommandError::Cancelled)
            } else {
                Ok(status)
            };
        }
        if cancel_state().is_cancelled() {
            break;
        }
        thread::sleep(POLL_INTERVAL);
    }

    if on_cancel == OnCancel::Kill {
        stop_child(child, grace, &cancel_state)?;
    }
    Err(CommandError::Cancelled)
}

/// Gives a cancelled child `grace` to exit, then kills it and waits until it is gone.
fn stop_child(
    child: &mut Child,
    grace: Duration,
    cancel_state: &impl Fn() -> CancelState,
) -> io::Result<()> {
    let deadline = Instant::now() + grace;
    let mut notified = false;

    while child.try_wait()?.is_none() {
        if cancel_state() == CancelState::Forced || Instant::now() >= deadline {
            // Fails only if the child exited in the meantime, which `wait` then reports.
            let _ = child.kill();
            child.wait()?;
            return Ok(());
        }
        if !notified {
            notified = true;
            cancel::print_notice(
                "Cancelling: waiting for Unity to exit. Press Ctrl+C again to stop it now.",
            );
        }
        thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

/// Continuously reads the log file and prints it to the console.
fn monitor_log_file(
    log_file: &Path,
    update_interval: Duration,
    stop_logging: &Arc<AtomicBool>,
) -> io::Result<()> {
    // Wait until the file exists.
    while !log_file.exists() {
        if stop_logging.load(Ordering::Acquire) {
            // If the file writer thread has finished without creating the file, we can stop waiting.
            return Ok(());
        }
        thread::sleep(update_interval);
    }

    let file = fs::File::open(log_file)?;
    let mut reader = io::BufReader::new(file);
    // The buffer can get quite large, pre-allocate a reasonable amount of memory.
    let mut buffer = Vec::with_capacity(128 * 1024);
    let mut ended_with_newline = false;

    loop {
        // Don't immediately exit if the file writer thread has finished to be able to read any last data.
        let should_stop = stop_logging.load(Ordering::Acquire);

        match reader.read_to_end(&mut buffer) {
            Err(e) if e.kind() != io::ErrorKind::Interrupted => return Err(e),
            _ => {}
        }

        let text = take_log_text(&mut buffer, should_stop);
        if !text.is_empty() {
            ended_with_newline = text.ends_with('\n');
            print!("{text}");
        }

        if should_stop {
            if !ended_with_newline {
                println!();
            }
            break;
        }
        thread::sleep(update_interval);
    }

    Ok(())
}

/// Takes the text read so far from `buffer`. Unity's log is UTF-8, but a plugin can write other
/// bytes; those become `U+FFFD`. A character whose last bytes have not been read yet stays in
/// the buffer, unless `at_end` says no more bytes will come.
fn take_log_text(buffer: &mut Vec<u8>, at_end: bool) -> String {
    let complete = if at_end {
        buffer.len()
    } else {
        buffer.len() - incomplete_char_len(buffer)
    };
    let text = String::from_utf8_lossy(&buffer[..complete]).into_owned();
    buffer.drain(..complete);
    text
}

/// The number of bytes at the end of `bytes` that start a UTF-8 character but do not finish it.
fn incomplete_char_len(bytes: &[u8]) -> usize {
    for (&byte, len) in bytes.iter().rev().take(3).zip(1..) {
        let char_len = match byte {
            0x80..=0xBF => continue, // A continuation byte: look further back.
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => return 0,
        };
        return if len < char_len { len } else { 0 };
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_text_waits_for_the_rest_of_a_split_character() {
        let mut buffer = b"caf\xC3".to_vec();
        assert_eq!(take_log_text(&mut buffer, false), "caf");
        assert_eq!(buffer, b"\xC3");

        buffer.extend_from_slice(b"\xA9\n");
        assert_eq!(take_log_text(&mut buffer, false), "\u{e9}\n");
        assert!(buffer.is_empty());
    }

    #[test]
    fn log_text_replaces_bytes_that_are_not_utf8() {
        // A Latin-1 'é', as a plugin might write it.
        let mut buffer = b"caf\xE9 au lait".to_vec();
        assert_eq!(take_log_text(&mut buffer, false), "caf\u{FFFD} au lait");

        // At the end, a character that was never finished is shown as it is.
        let mut buffer = b"caf\xE9".to_vec();
        assert_eq!(take_log_text(&mut buffer, false), "caf");
        assert_eq!(take_log_text(&mut buffer, true), "\u{FFFD}");
    }

    #[test]
    fn a_spawn_error_has_no_exit_code() {
        // Error code 2 is "file not found" on Unix and Windows, and must not be mistaken for a
        // process that exited with code 2.
        let error = CommandError::from(io::Error::from_raw_os_error(2));
        assert_eq!(error.exit_code(), None);
    }

    #[test]
    fn a_failed_process_keeps_its_exit_code() {
        let cmd = if cfg!(windows) {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", "exit 2"]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", "exit 2"]);
            cmd
        };
        let error = wait_with_stdout(cmd, OnCancel::Kill).unwrap_err();
        assert_eq!(error.exit_code(), Some(2));
    }

    /// Starts a process that runs for `seconds` unless it is stopped.
    fn spawn_sleeping(seconds: u32) -> Child {
        let mut cmd = if cfg!(windows) {
            let mut cmd = Command::new("powershell");
            cmd.args(["-NoProfile", "-Command", &format!("Start-Sleep {seconds}")]);
            cmd
        } else {
            let mut cmd = Command::new("sleep");
            cmd.arg(seconds.to_string());
            cmd
        };
        cmd.stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    const LONG: Duration = Duration::from_secs(60);

    #[test]
    fn wait_for_child_returns_the_status_when_not_cancelled() {
        let mut child = spawn_sleeping(0);
        let status = wait_for_child(&mut child, OnCancel::Kill, LONG, || CancelState::Running);
        assert!(status.unwrap().success());
    }

    #[test]
    fn a_detached_child_keeps_running_after_cancel() {
        let mut child = spawn_sleeping(60);
        let result = wait_for_child(&mut child, OnCancel::Detach, LONG, || {
            CancelState::Requested
        });

        assert!(matches!(result, Err(CommandError::Cancelled)));
        assert!(child.try_wait().unwrap().is_none());
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn a_cancelled_child_is_killed_after_the_grace_period() {
        let mut child = spawn_sleeping(60);
        let start = Instant::now();
        let result = wait_for_child(
            &mut child,
            OnCancel::Kill,
            Duration::from_millis(200),
            || CancelState::Requested,
        );

        assert!(matches!(result, Err(CommandError::Cancelled)));
        assert!(child.try_wait().unwrap().is_some());
        assert!(start.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_forced_cancel_kills_without_waiting_for_the_grace_period() {
        let mut child = spawn_sleeping(60);
        let start = Instant::now();
        let result = wait_for_child(&mut child, OnCancel::Kill, LONG, || CancelState::Forced);

        assert!(matches!(result, Err(CommandError::Cancelled)));
        assert!(child.try_wait().unwrap().is_some());
        assert!(start.elapsed() < Duration::from_secs(10));
    }

    #[cfg(unix)]
    #[test]
    fn a_cancelled_child_that_exits_in_the_grace_period_is_not_killed() {
        let mut child = spawn_sleeping(1);
        let result = wait_for_child(&mut child, OnCancel::Kill, LONG, || CancelState::Requested);

        assert!(matches!(result, Err(CommandError::Cancelled)));
        // A killed process has no exit code.
        assert!(child.try_wait().unwrap().unwrap().success());
    }

    #[test]
    fn a_child_that_exits_on_the_same_ctrl_c_counts_as_cancelled() {
        let mut child = spawn_sleeping(0);
        child.wait().unwrap();
        let result = wait_for_child(&mut child, OnCancel::Kill, LONG, || CancelState::Requested);
        assert!(matches!(result, Err(CommandError::Cancelled)));
    }

    #[test]
    fn a_cancellation_leads_main_to_the_cancel_exit_code() {
        let error = anyhow::Error::from(CommandError::Cancelled).context("Cannot build");
        assert!(matches!(
            crate::utils::ReportedFailure::find(&error),
            Some(crate::utils::ReportedFailure::Interrupted)
        ));
    }
}
