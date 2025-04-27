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
pub struct CommandError {
    pub exit_code: i32,
    pub stderr: String,
}

impl Error for CommandError {}

impl From<std::process::Output> for CommandError {
    fn from(output: std::process::Output) -> Self {
        let exit_code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8(output.stderr).unwrap_or_else(|e| {
            // If the output is not valid UTF-8 convert it.
            // TODO: Unfortunately, this rechecks the UTF-8 validity. Atm there is no stable function to avoid this.
            String::from_utf8_lossy(e.as_bytes()).into_owned()
        });

        Self { exit_code, stderr }
    }
}

impl From<io::Error> for CommandError {
    fn from(err: io::Error) -> Self {
        Self {
            exit_code: err.raw_os_error().unwrap_or(-1),
            stderr: err.to_string(),
        }
    }
}

impl Display for CommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Command failed with exit code: {} {}",
            self.exit_code, self.stderr
        )
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
    log_monitor.join().map_err(|e| CommandError {
        exit_code: -1,
        stderr: format!("Echo runner thread panicked: {e:?}"),
    })??;

    let output = output?;
    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| output.into())
}

/// Spawns command and immediately returns without any output.
pub fn spawn_and_forget(mut cmd: Command) -> Result<(), CommandError> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map(|_| ())
        .map_err(std::convert::Into::into)
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
