use anyhow::Result;
use std::process::Command;

/// A runner for commands.
pub(crate) struct CmdRunner {
    command: Command,
    description: String,
}

impl CmdRunner {
    pub(crate) fn new(command: Command, description: String) -> Self {
        CmdRunner {
            command,
            description,
        }
    }

    pub(crate) fn run(self, wait: bool, quiet: bool, dry_run: bool) -> Result<()> {
        if dry_run {
            println!("{}", command_line_string(&self.command));
            return Ok(());
        }

        if !quiet {
            println!("{}", self.description);
        }

        let output = execute_command(self.command, wait)?;
        if !output.is_empty() {
            print!("{}", output);
        }
        Ok(())
    }
}

/// Executes the command and returns the output.
/// Returns empty output if command is not awaited.
fn execute_command(mut cmd: Command, wait: bool) -> Result<String> {
    if !wait {
        let _ = cmd.spawn()?;
        return Ok("".to_string());
    }

    let output = cmd.output()?;
    String::from_utf8(output.stdout).map_err(Into::into)
}

/// Returns the command as a full command line string.
fn command_line_string(cmd: &Command) -> String {
    let mut line = cmd.get_program().to_string_lossy().to_string();

    // Handle spaces in path.
    if line.contains(' ') {
        if cfg!(target_os = "macos") {
            line = format!("'{}'", line);
        } else if cfg!(target_os = "windows") {
            line = format!("& '{}'", line);
        }
    }

    for arg in cmd.get_args() {
        let mut arg = arg.to_string_lossy().to_string();
        // Handle spaces in arguments.
        if arg.contains(' ') {
            arg = format!("'{}'", arg);
        }

        line.push_str(&format!(" {}", arg));
    }
    line
}
