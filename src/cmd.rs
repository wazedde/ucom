use std::borrow::Cow;
use std::process::Command;

use anyhow::Result;

pub(crate) type FnCmdAction = dyn FnOnce() -> Result<()>;

/// A runner for commands.
pub(crate) struct CmdRunner<'a> {
    /// The system command to perform.
    command: Command,
    /// An optional action that is performed before the command.
    pre_action: Option<Box<FnCmdAction>>,
    /// An optional action that is performed before the command.
    post_action: Option<Box<FnCmdAction>>,
    /// A description of the command.
    description: Cow<'a, str>,
}

impl<'a> CmdRunner<'a> {
    /// Creates a new command runner.
    ///
    /// # Arguments
    ///
    /// * `command`: The system command to perform.
    /// * `pre_action`: An optional action that is performed before the command.
    /// * `description`: A description of the command.
    ///
    /// returns: CmdRunner
    pub(crate) fn new<S>(
        command: Command,
        pre_action: Option<Box<FnCmdAction>>,
        post_action: Option<Box<FnCmdAction>>,
        description: S,
    ) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        CmdRunner {
            command,
            pre_action,
            post_action,
            description: description.into(),
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
            let output = cmd.output();

            let output = output?;

            if let Some(post_action) = self.post_action {
                post_action()?;
            }

            let stdout = String::from_utf8(output.stdout)?;

            print!("{}", stdout);
            if !stdout.ends_with('\n') {
                println!();
            }

            output.status.success().then_some(()).ok_or_else(|| {
                anyhow::anyhow!(
                    "Command failed with exit code: {}",
                    output.status.code().unwrap_or(-1)
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
    fn to_command_line_string(&self) -> String {
        let mut line = self.command.get_program().to_string_lossy().to_string();

        // Handle spaces in path.
        if line.contains(' ') {
            if cfg!(target_os = "macos") {
                line = format!("\"{}\"", line);
            } else if cfg!(target_os = "windows") {
                line = format!("& \"{}\"", line);
            }
        }

        for arg in self.command.get_args() {
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
}
