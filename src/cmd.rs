use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, io, thread};

use anyhow::{anyhow, Result};

/// Spawns command and immediately returns.
pub(crate) fn forget_command(cmd: Command) -> Result<()> {
    spawn_command(cmd).map(|_| ())
}

/// Spawns command and outputs to the console.
/// Returns when the command finishes.
pub(crate) fn run_command_to_stdout(cmd: Command) -> Result<()> {
    let child = spawn_command(cmd)?;

    let output = child
        .wait_with_output()
        .map_err(|e| anyhow!("Failed to wait for child process: {}", e));

    let output = output?;
    let stdout = String::from_utf8(output.stdout)?;
    print!("{}", stdout);

    let stderr = String::from_utf8(output.stderr)?;
    output.status.success().then_some(()).ok_or_else(|| {
        anyhow!(
            "Command failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        )
    })
}

/// Spawns command and outputs Unity's log to the console.
/// Returns when the command finishes.
pub(crate) fn run_command_with_log_capture(cmd: Command, log_file: &Path) -> Result<()> {
    let child = spawn_command(cmd)?;

    let is_finished = Arc::new(Mutex::new(false));
    let moved_is_finished = Arc::clone(&is_finished);
    let moved_path = log_file.to_path_buf();

    let log_reader = thread::spawn(move || {
        continuous_log_reader(&moved_path, Duration::from_millis(100), moved_is_finished);
    });

    let output = child
        .wait_with_output()
        .map_err(|e| anyhow!("Failed to wait for child process: {}", e));

    *is_finished.lock().unwrap() = true;

    // Wait for reader to finish.
    log_reader.join().expect("Log reader thread panicked");

    let output = output?;
    let stderr = String::from_utf8(output.stderr)?;
    output.status.success().then_some(()).ok_or_else(|| {
        anyhow!(
            "Command failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        )
    })
}

/// Returns the command as a full command line string.
pub(crate) fn to_command_line_string(cmd: &Command) -> String {
    let mut line = cmd.get_program().to_string_lossy().to_string();

    // Handle spaces in path.
    if line.contains(' ') {
        if cfg!(target_os = "macos") {
            line = format!("\"{}\"", line);
        } else if cfg!(target_os = "windows") {
            line = format!("& \"{}\"", line);
        }
    }

    for arg in cmd.get_args() {
        let arg = arg.to_string_lossy();
        // Handle spaces in arguments.
        if arg.contains(' ') {
            line.push_str(&format!(" \"{}\"", arg));
        } else {
            line.push_str(&format!(" {}", arg));
        }
    }
    line
}

fn spawn_command(mut cmd: Command) -> Result<Child> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to run child process: {}", e))
}

fn continuous_log_reader(
    log_file: &Path,
    read_interval: Duration,
    finish_reading: Arc<Mutex<bool>>,
) {
    // Wait until file exists.
    while !log_file.exists() {
        if *finish_reading.lock().unwrap() {
            // If the file writer thread is finished without creating the file, then we can stop waiting.
            return;
        }
        thread::sleep(read_interval);
    }

    let file = fs::File::open(log_file).unwrap();
    let mut file = io::BufReader::new(file);
    let mut buf = String::new();
    let mut ended_with_newline = false;

    loop {
        let is_finished = *finish_reading.lock().unwrap();

        file.read_to_string(&mut buf).unwrap();
        if !buf.is_empty() {
            ended_with_newline = buf.ends_with('\n');
            print!("{}", buf);
            buf.clear();
        }

        // Break when other thread has finished.
        if is_finished {
            if !ended_with_newline {
                println!();
            }
            break;
        }
        thread::sleep(read_interval);
    }
}
