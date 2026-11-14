use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{fs, io, thread};

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
}

impl CommandError {
    /// The exit code of a command that ran and failed. `None` if it could not be started, or
    /// was ended by a signal.
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Failed { code, .. } => *code,
            Self::Io(_) => None,
        }
    }
}

// No `source`: the message already includes the I/O error, which would otherwise be printed
// twice in an error chain.
impl Error for CommandError {}

impl From<std::process::Output> for CommandError {
    fn from(output: std::process::Output) -> Self {
        let stderr = String::from_utf8(output.stderr).unwrap_or_else(|e| {
            // If the output is not valid UTF-8 convert it.
            // TODO: Unfortunately, this rechecks the UTF-8 validity. Atm there is no stable function to avoid this.
            String::from_utf8_lossy(e.as_bytes()).into_owned()
        });

        Self::Failed {
            code: output.status.code(),
            stderr,
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

/// Spawns command and outputs Unity's log to the console. Blocks until the command has finished.
pub fn wait_with_log_output(mut cmd: Command, log_file: &Path) -> Result<(), CommandError> {
    let child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let stop_monitoring = Arc::new(AtomicBool::new(false));

    let log_monitor = thread::spawn({
        let stop_monitoring = stop_monitoring.clone();
        let log_file = log_file.to_owned();
        move || monitor_log_file(&log_file, Duration::from_millis(100), &stop_monitoring)
    });

    let output = child.wait_with_output();
    stop_monitoring.store(true, Ordering::Release);

    // Wait for the log monitor thread to finish.
    log_monitor
        .join()
        .map_err(|e| io::Error::other(format!("Echo runner thread panicked: {e:?}")))??;

    let output = output?;
    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| output.into())
}

/// Spawns command and immediately returns without any output.
///
/// Output is discarded via the null device rather than unread pipes, so the
/// child never writes into a closed pipe after the handles are dropped.
pub fn spawn_and_forget(mut cmd: Command) -> Result<(), CommandError> {
    cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    Ok(())
}

/// Spawns command and outputs to the console. Blocks until the command has finished.
pub fn wait_with_stdout(mut cmd: Command) -> Result<(), CommandError> {
    let child = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let output = child.wait_with_output()?;

    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| output.into())
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
    let mut buffer = String::with_capacity(128 * 1024);
    let mut ended_with_newline = false;

    loop {
        // Don't immediately exit if the file writer thread has finished to be able to read any last data.
        let should_stop = stop_logging.load(Ordering::Acquire);

        match reader.read_to_string(&mut buffer) {
            Ok(_) if !buffer.is_empty() => {
                ended_with_newline = buffer.ends_with('\n');
                print!("{buffer}");
                buffer.clear();
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                // Ignore interrupted errors.
            }
            Err(e) => {
                return Err(e);
            }
            _ => {}
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let error = wait_with_stdout(cmd).unwrap_err();
        assert_eq!(error.exit_code(), Some(2));
    }
}
