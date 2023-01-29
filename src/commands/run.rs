use std::process::Command;

use colored::Colorize;

use crate::cli::RunArguments;
use crate::unity::{cmd_to_string, matching_editor, spawn_and_forget, wait_with_stdout};

/// Runs the Unity Editor with the given arguments.
pub fn run_unity(arguments: RunArguments) -> anyhow::Result<()> {
    let (version, editor_exe) = matching_editor(arguments.version_pattern.as_deref())?;

    let mut cmd = Command::new(editor_exe);
    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", cmd_to_string(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println!("{}", format!("Run Unity {version}").bold());
    }

    if arguments.wait {
        wait_with_stdout(cmd)
    } else {
        spawn_and_forget(cmd)
    }
}
