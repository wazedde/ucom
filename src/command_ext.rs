use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, io, thread};

use anyhow::{anyhow, Context, Result};

pub trait CommandExt {
    /// Spawns command and immediately returns without any output.
    fn forget(self) -> Result<()>;

    /// Spawns command and outputs Unity's log to the console.
    /// Returns when the command has finished.
    fn wait_with_log_echo(self, log_file: &Path) -> Result<()>;

    /// Spawns command and outputs to the console.
    /// Returns when the command has finished.
    fn wait_with_stdout(self) -> Result<()>;

    /// Returns the full command line string.
    fn to_command_line_string(&self) -> String;
}

impl CommandExt for Command {
    fn forget(mut self) -> Result<()> {
        self.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map(|_| ())
            .context("Failed to run child process.")
    }

    fn wait_with_log_echo(mut self, log_file: &Path) -> Result<()> {
        let child = self
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to run child process.")?;

        let stop_echo = Arc::new(Mutex::new(false));

        let echo_closure = {
            let stop_echo = Arc::clone(&stop_echo);
            let log_file = log_file.to_owned();
            move || echo_log_file(&log_file, Duration::from_millis(100), &stop_echo)
        };

        let echo_runner = thread::spawn(echo_closure);

        let output = child
            .wait_with_output()
            .context("Failed to wait for child process.");

        *stop_echo.lock().unwrap() = true;

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

    fn wait_with_stdout(mut self) -> Result<()> {
        let child = self
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

    fn to_command_line_string(&self) -> String {
        let mut line = self.get_program().to_string_lossy().to_string();

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

        for arg in self.get_args() {
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
}

fn echo_log_file(log_file: &Path, update_interval: Duration, stop_thread: &Arc<Mutex<bool>>) {
    // Wait until file exists.
    while !log_file.exists() {
        if *stop_thread.lock().unwrap() {
            // If the file writer thread has finished without creating the file we can stop waiting.
            return;
        }
        thread::sleep(update_interval);
    }

    let file = fs::File::open(log_file).expect("Cannot open log file.");
    let mut reader = io::BufReader::new(file);
    let mut buffer = String::new();
    let mut ended_with_newline = false;

    loop {
        // Don't immediately exit if the file writer thread has finished to be able to read any last data.
        let should_stop = *stop_thread.lock().unwrap();

        reader.read_to_string(&mut buffer).unwrap();
        if !buffer.is_empty() {
            ended_with_newline = buffer.ends_with('\n');
            print!("{buffer}");
            buffer.clear();
        }

        // Now do the actual check if we should stop.
        if should_stop {
            if !ended_with_newline {
                println!();
            }
            break;
        }
        thread::sleep(update_interval);
    }
}
