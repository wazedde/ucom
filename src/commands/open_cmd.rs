use crate::cli::OpenArguments;
use crate::commands::{UnityCommandBuilder, check_version_issues, execute_unity_command};
use crate::unity::installations::Installations;
use crate::unity::{ProjectPath, build_command_line};
use crate::utils::path_ext::PlatformConsistentPathExt;

/// Opens the given Unity project in the Unity Editor.
pub fn open_project(arguments: OpenArguments) -> anyhow::Result<()> {
    let project = ProjectPath::try_from(&arguments.project_dir)?;
    let project_unity_version = project.unity_version()?;

    let open_unity_version = match arguments.upgrade_version {
        // If a specific version is given, use that.
        Some(Some(pattern)) => Installations::latest_installed_version(Some(&pattern)),
        // Otherwise, use the latest version.
        Some(None) => Installations::latest_installed_version(Some(
            &project_unity_version.to_major_minor_string(),
        )),
        // Otherwise, use the current version.
        None => Ok(project_unity_version),
    }?;

    let editor_exe = open_unity_version.editor_executable_path()?;

    project.ensure_assets_directory_exists()?;

    // Build the command to execute.
    let mut builder = UnityCommandBuilder::new(editor_exe)
        .with_project_path(project.to_path_buf())
        .quit(arguments.quit)
        .add_args(arguments.args.unwrap_or_default());

    if let Some(target) = arguments.target {
        builder = builder.with_build_target(target.as_ref());
    }

    let cmd = builder.build();

    if arguments.dry_run {
        println!("{}", build_command_line(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println!(
            "Open Unity {v} project in: {p}",
            v = open_unity_version,
            p = project.normalized_display()
        );
        check_version_issues(open_unity_version);
    }

    execute_unity_command(cmd, arguments.wait, arguments.quiet)
}
