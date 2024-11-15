use std::process::Command;

use crate::cli_run::RunArguments;
use crate::commands::println_b;
use crate::unity::installed::VersionList;
use crate::unity::*;

/// Runs the Unity Editor with the given arguments.
pub(crate) fn run_unity(arguments: RunArguments) -> anyhow::Result<()> {
    let unity_version = VersionList::latest(Some(&arguments.version_pattern))?;
    let editor_exe = unity_version.editor_executable_path()?;

    let mut cmd = Command::new(editor_exe);
    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", build_command_line(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println_b!("Run Unity {unity_version}");
    }

    if arguments.wait {
        wait_with_stdout(cmd)?;
    } else {
        spawn_and_forget(cmd);
    }
    Ok(())
}
