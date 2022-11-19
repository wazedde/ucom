use std::process::Command;

use anyhow::Result;

/// A runner for commands.
pub(crate) struct CmdRunner {
    command: Command,
    description: String,
}

impl CmdRunner {
    pub(crate) fn new(command: Command, description: String) -> Self {
        CmdRunner { command, description }
    }

    pub(crate) fn run(self, wait: bool, quiet: bool, dry_run: bool) -> Result<()> {
        if dry_run {
            println!("{}", command_string(&self.command));
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
fn execute_command(mut cmd: Command, wait: bool) -> Result<String> {
    if !wait {
        cmd = wrap_in_unwaited_command(&cmd);
    }

    let process = cmd.output()?;
    String::from_utf8(process.stdout).map_err(Into::into)
}

/// Returns the command as String.
fn command_string(cmd: &Command) -> String {
    let mut line = cmd.get_program().to_string_lossy().to_string();

    for arg in cmd.get_args() {
        line.push_str(&format!(" {}", arg.to_string_lossy()));
    }

    line
}

/// Wraps the command in system specif command that not waits for thw command to finish.
fn wrap_in_unwaited_command(cmd: &Command) -> Command {
    if cfg!(target_os = "macos") {
        let mut wrapped = Command::new("open");
        wrapped
            .args(["-na", &cmd.get_program().to_string_lossy()])
            .arg("--args")
            .args(cmd.get_args());

        wrapped
    } else if cfg!(target_os = "windows") {
        todo!()
    } else {
        unimplemented!()
    }
}
