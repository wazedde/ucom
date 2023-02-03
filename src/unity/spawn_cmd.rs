use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, io, thread};

use anyhow::{anyhow, Context, Result};

/// Returns the full command line string.
pub fn cmd_to_string(cmd: &Command) -> String {
    let mut line = cmd.get_program().to_string_lossy().to_string();

    // Handle spaces in path.
    if line.contains(char::is_whitespace) {
        if cfg!(target_os = "macos") {
            line = format!("\"{line}\"");
        } else if cfg!(target_os = "windows") {
            line = format!("& \"{line}\"");
        } else {
            unimplemented!();
        }
    }

    for arg in cmd.get_args() {
        line.push(' ');
        let arg = arg.to_string_lossy();
        // Handle spaces in arguments.
        if arg.contains(char::is_whitespace) {
            line.push_str(&format!("\"{arg}\""));
        } else {
            line.push_str(&arg);
        }
    }
    line
}

/// Spawns command and outputs Unity's log to the console. Blocks until the command has finished.
pub fn wait_with_log_output(mut cmd: Command, log_file: &Path) -> Result<()> {
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to run child process.")?;

    let build_finished = Arc::new(AtomicBool::new(false));

    let echo_log = {
        let build_finished = build_finished.clone();
        let log_file = log_file.to_owned();
        move || echo_log_file(&log_file, Duration::from_millis(100), &build_finished)
    };

    let echo_runner = thread::spawn(echo_log);

    let output = child
        .wait_with_output()
        .context("Failed to wait for child process.");

    build_finished.store(true, Ordering::SeqCst);

    // Wait for echo to finish.
    echo_runner.join().expect("Log echo thread panicked.");

    let output = output?;
    output.status.success().then_some(()).ok_or_else(|| {
        anyhow!(
            "Command failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8(output.stderr).unwrap_or_default()
        )
    })
}

/// Spawns command and immediately returns without any output.
pub fn spawn_and_forget(mut cmd: Command) -> Result<()> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map(|_| ())
        .context("Failed to run child process.")
}

/// Spawns command and outputs to the console. Blocks until the command has finished.
pub fn wait_with_stdout(mut cmd: Command) -> Result<()> {
    let child = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to run child process.")?;

    let output = child
        .wait_with_output()
        .context("Failed to wait for child process.")?;

    output.status.success().then_some(()).ok_or_else(|| {
        anyhow!(
            "Command failed with exit code {}",
            output.status.code().unwrap_or(-1)
        )
    })
}

/// Continuously reads the log file and prints it to the console.
fn echo_log_file(log_file: &Path, update_interval: Duration, stop_logging: &Arc<AtomicBool>) {
    // Wait until file exists.
    while !log_file.exists() {
        if stop_logging.load(Ordering::SeqCst) {
            // If the file writer thread has finished without creating the file we can stop waiting.
            return;
        }
        thread::sleep(update_interval);
    }

    let file = fs::File::open(log_file).expect("Cannot open log file.");
    let mut reader = io::BufReader::new(file);
    // The buffer can get quite large, pre-allocate a reasonable amount of memory.
    let mut buffer = String::with_capacity(128 * 1024);
    let mut ended_with_newline = false;

    loop {
        // Don't immediately exit if the file writer thread has finished to be able to read any last data.
        let should_stop = stop_logging.load(Ordering::SeqCst);

        reader.read_to_string(&mut buffer).unwrap();
        if !buffer.is_empty() {
            ended_with_newline = buffer.ends_with('\n');
            print!("{buffer}");
            buffer.clear();
        }

        if should_stop {
            if !ended_with_newline {
                println!();
            }
            break;
        }
        thread::sleep(update_interval);
    }
}
