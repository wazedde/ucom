use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, io, thread};

use anyhow::{anyhow, Result};

pub(crate) trait CommandExt {
    /// Spawns command and immediately returns without any output.
    fn forget(self) -> Result<()>;

    /// Spawns command and outputs Unity's log to the console.
    /// Returns when the command has finished.
    fn wait_with_log_capture(self, log_file: &Path) -> Result<()>;

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
            .map_err(|e| anyhow!("Failed to run child process: {}", e))
    }

    fn wait_with_log_capture(mut self, log_file: &Path) -> Result<()> {
        let child = self
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to run child process: {}", e))?;

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
        output.status.success().then_some(()).ok_or_else(|| {
            anyhow!(
                "Command failed with exit code {}: {}",
                output.status.code().unwrap_or(-1),
                String::from_utf8(output.stderr).unwrap_or_default()
            )
        })
    }

    fn wait_with_stdout(mut self) -> Result<()> {
        // let child = spawn_command(self)?;
        let child = self
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| anyhow!("Failed to run child process: {}", e))?;

        let output = child
            .wait_with_output()
            .map_err(|e| anyhow!("Failed to wait for child process: {}", e))?;

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
                line = format!("\"{}\"", line);
            } else if cfg!(target_os = "windows") {
                line = format!("& \"{}\"", line);
            } else {
                unimplemented!();
            }
        }

        for arg in self.get_args() {
            line.push(' ');
            let arg = arg.to_string_lossy();
            // Handle spaces in arguments.
            if arg.contains(char::is_whitespace) {
                line.push_str(&format!("\"{}\"", arg));
            } else {
                line.push_str(&arg);
            }
        }
        line
    }
}

fn continuous_log_reader(
    log_file: &Path,
    update_interval: Duration,
    finish_reading: Arc<Mutex<bool>>,
) {
    // Wait until file exists.
    while !log_file.exists() {
        if *finish_reading.lock().unwrap() {
            // If the file writer thread is finished without creating the file, then we can stop waiting.
            return;
        }
        thread::sleep(update_interval);
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
        thread::sleep(update_interval);
    }
}
