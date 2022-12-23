use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{fs, io, thread};

use anyhow::{anyhow, Result};

use crate::OptionalFn;

/// A runner for commands.
pub(crate) struct CmdRunner {
    /// The system command to perform.
    command: Command,
    /// An optional action that is performed before the command.
    pre_action: OptionalFn,
    /// An optional action that is performed before the command.
    post_action: OptionalFn,
    /// A description of the command.
    description: String,
}

impl CmdRunner {
    /// Creates a new command runner.
    ///
    /// # Arguments
    ///
    /// * `command`: The system command to perform.
    /// * `pre_action`: An optional action that is performed before the command.
    /// * `description`: A description of the command.
    ///
    /// returns: CmdRunner
    pub(crate) fn new(
        command: Command,
        pre_action: OptionalFn,
        post_action: OptionalFn,
        description: String,
    ) -> Self {
        CmdRunner {
            command,
            pre_action,
            post_action,
            description,
        }
    }

    /// Runs the command.
    ///
    /// # Arguments
    ///
    /// * `wait`: If true, the command will be run synchronously.
    /// * `quiet`: If true, the command will not print any output.
    /// * `dry_run`: If true, the command will not be run.
    ///
    /// returns: Result<(), Error>
    pub(crate) fn run(self, wait: bool, quiet: bool, dry_run: bool) -> Result<()> {
        if dry_run {
            println!("{}", self.to_command_line_string());
            return Ok(());
        };

        if !quiet {
            println!("{}", self.description);
        }

        if let Some(pre_action) = self.pre_action {
            pre_action()?;
        }

        let mut cmd = self.command;

        if wait {
            let child = cmd
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| anyhow!("Failed to run child process: {}", e))?;

            let output = child
                .wait_with_output()
                .map_err(|e| anyhow!("Failed to wait for child process: {}", e));

            // Run the post action.
            if let Some(post_action) = self.post_action {
                post_action()?;
            }

            let output = output?;
            let _stdout = String::from_utf8(output.stdout)?;
            let stderr = String::from_utf8(output.stderr)?;

            output.status.success().then_some(()).ok_or_else(|| {
                anyhow!(
                    "Command failed with exit code {}: {}",
                    output.status.code().unwrap_or(-1),
                    stderr
                )
            })?;
        } else {
            let _ = cmd.spawn()?;
            // todo: do something with post action?
        }
        Ok(())
    }

    /// Returns the command as a full command line string.
    ///
    /// # Arguments
    ///
    /// * `cmd`: The command to convert to a command line string.
    ///
    /// returns: String
    pub(crate) fn to_command_line_string(&self) -> String {
        to_command_line_string(&self.command)
    }
}

pub(crate) fn spawn_command(mut cmd: Command) -> Result<Child> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to run child process: {}", e))
}

pub(crate) fn run_command_with_stdout(cmd: Command) -> Result<()> {
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

pub(crate) fn run_command_with_log_output(cmd: Command, log_file: &Path) -> Result<()> {
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
    let _stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    output.status.success().then_some(()).ok_or_else(|| {
        anyhow!(
            "Command failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        )
    })
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

    loop {
        let is_finished = *finish_reading.lock().unwrap();

        file.read_to_string(&mut buf).unwrap();
        if !buf.is_empty() {
            print!("{}", buf);
            buf.clear();
        }

        // Break when other thread has finished.
        if is_finished {
            break;
        }
        thread::sleep(read_interval);
    }
}

/// Returns the command as a full command line string.
///
/// # Arguments
///
/// * `cmd`: The command to convert to a command line string.
///
/// returns: String
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
