use std::process::Command;

use yansi::Paint;

use crate::cli::*;
use crate::unity::installed::InstalledVersions;
use crate::unity::*;

/// Opens the given Unity project in the Unity Editor.
pub fn open_project(arguments: OpenArguments) -> anyhow::Result<()> {
    let project = ProjectPath::try_from(&arguments.project_dir)?;
    let project_unity_version = project.unity_version()?;

    let open_unity_version = match arguments.upgrade_version {
        // If a specific version is given, use that.
        Some(Some(pattern)) => InstalledVersions::latest(Some(&pattern)),
        // Otherwise, use the latest version.
        Some(None) => InstalledVersions::latest(Some(&project_unity_version.minor_partial())),
        // Otherwise, use the current version.
        None => Ok(project_unity_version),
    }?;

    let editor_exe = open_unity_version.editor_executable_path()?;

    project.check_assets_directory_exists()?;

    // Build the command to execute.
    let mut cmd = Command::new(editor_exe);
    cmd.args(["-projectPath", &project.as_path().to_string_lossy()]);

    if let Some(target) = arguments.target {
        cmd.args(["-buildTarget", target.as_ref()]);
    }

    if arguments.quit {
        cmd.arg("-quit");
    }

    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", build_command_line(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        let s = format!(
            "Open Unity {} project in: {}",
            open_unity_version,
            project.as_path().display()
        );
        println!("{}", s.bold());
    }

    if arguments.wait {
        wait_with_stdout(cmd)?;
    } else {
        spawn_and_forget(cmd);
    }
    Ok(())
}
