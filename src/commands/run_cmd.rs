use std::process::Command;

use crate::cli_run::RunArguments;
use crate::commands::execute_unity_command;
use crate::unity::build_command_line;
use crate::unity::installations::Installations;

/// Runs the Unity Editor with the given arguments.
pub fn run_unity(arguments: RunArguments) -> anyhow::Result<()> {
    let unity_version = Installations::latest_installed_version(Some(&arguments.version_pattern))?;
    let editor_exe = unity_version.editor_executable_path()?;

    let mut run_command = Command::new(editor_exe);
    run_command.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", build_command_line(&run_command));
        return Ok(());
    }

    if !arguments.quiet {
        println!("Run Unity {unity_version}");
    }

    execute_unity_command(run_command, arguments.wait, arguments.quiet)
}
