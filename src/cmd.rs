use std::process::Command;

use anyhow::Result;

pub(crate) type FnCmdAction = Box<dyn FnOnce() -> Result<()>>;

/// A runner for commands.
pub(crate) struct CmdRunner {
    /// The system command to perform.
    command: Command,
    /// An optional action that is performed before the command.
    pre_action: Option<FnCmdAction>,
    /// A description of the command.
    description: String,
}

impl CmdRunner {
    pub(crate) fn new(
        command: Command,
        pre_action: Option<FnCmdAction>,
        description: String,
    ) -> Self {
        CmdRunner {
            command,
            pre_action,
            description,
        }
    }

    pub(crate) fn run(self, wait: bool, quiet: bool, dry_run: bool) -> Result<()> {
        if dry_run {
            println!("{}", to_line_string(&self.command));
            return Ok(());
        }

        if !quiet {
            println!("{}", self.description);
        }

        if let Some(pre_action) = self.pre_action {
            pre_action()?;
        }

        let mut cmd = self.command;

        if wait {
            let output = cmd.output()?;
            let stdout = String::from_utf8(output.stdout)?;

            print!("{}", stdout);
            if !stdout.ends_with('\n') {
                println!();
            }
        } else {
            let _ = cmd.spawn()?;
        }
        Ok(())
    }
}

/// Returns the command as a full command line string.
fn to_line_string(cmd: &Command) -> String {
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
        let arg = arg.to_string_lossy();
        // Handle spaces in arguments.
        if arg.contains(' ') {
            line.push_str(&format!(" '{}'", arg));
        } else {
            line.push_str(&format!(" {}", arg));
        }
    }
    line
}
