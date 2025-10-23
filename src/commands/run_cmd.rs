use crate::cli_run::RunArguments;
use crate::commands::{UnityCommandBuilder, execute_unity_command};
use crate::unity::build_command_line;
use crate::unity::installations::Installations;

/// Runs the Unity Editor with the given arguments.
pub fn run_unity(arguments: RunArguments) -> anyhow::Result<()> {
    let unity_version = Installations::latest_installed_version(Some(&arguments.version_pattern))?;
    let editor_exe = unity_version.editor_executable_path()?;

    let run_command = UnityCommandBuilder::new(editor_exe)
        .add_args(arguments.args.unwrap_or_default())
        .build();

    if arguments.dry_run {
        println!("{}", build_command_line(&run_command));
        return Ok(());
    }

    if !arguments.quiet {
        println!("Run Unity {unity_version}");
    }

    execute_unity_command(run_command, arguments.wait, arguments.quiet)
}
