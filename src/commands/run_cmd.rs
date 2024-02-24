use std::process::Command;

use colored::Colorize;

use crate::cli_run::RunArguments;
use crate::unity::*;

/// Runs the Unity Editor with the given arguments.
pub fn run_unity(arguments: RunArguments) -> anyhow::Result<()> {
    let unity_version = latest_installed_version(arguments.version_pattern.as_deref())?;
    let editor_exe = unity_version.editor_executable_path()?;

    let mut cmd = Command::new(editor_exe);
    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", build_command_line(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println!("{}", format!("Run Unity {unity_version}").bold());
    }

    if arguments.wait {
        wait_with_stdout(cmd)?;
    } else {
        spawn_and_forget(cmd);
    }
    Ok(())
}
