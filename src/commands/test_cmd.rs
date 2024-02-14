use std::process::Command;

use chrono::prelude::*;
use colored::Colorize;

use crate::cli::TestArguments;
use crate::commands::terminal_spinner::TerminalSpinner;
use crate::unity::{build_command_line, wait_with_stdout, ProjectPath};

pub fn run_tests(arguments: TestArguments) -> anyhow::Result<()> {
    let project = ProjectPath::try_from(&arguments.project_dir)?;
    let project_unity_version = project.unity_version()?;
    let editor_exe = project_unity_version.editor_executable_path()?;
    project.check_assets_directory_exists()?;

    // Build the command to execute.
    let mut cmd = Command::new(editor_exe);
    cmd.args(["-projectPath", &project.as_path().to_string_lossy()]);
    cmd.arg("-runTests");
    cmd.args(["-testPlatform", &arguments.platform.to_string()]);

    if arguments.batch_mode {
        cmd.arg("-batchmode");
    }

    if arguments.forget_project_path {
        cmd.arg("-forgetProjectPath");
    }

    if let Some(s) = arguments.categories {
        cmd.args(["-testCategory", &format!("\"{s}\"")]);
    }

    if let Some(s) = arguments.tests {
        cmd.args(["-testFilter", &format!("\"{s}\"")]);
    }

    if let Some(s) = arguments.assemblies {
        cmd.args(["-assemblyNames", &format!("\"{s}\"")]);
    }

    let timestamp = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let filename = format!("tests-{}-{}.xml", arguments.platform, timestamp);
    let output_path = project.as_path().join(filename);
    cmd.args(["-testResults", &output_path.to_string_lossy()]);

    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", build_command_line(&cmd));
        return Ok(());
    }

    let spinner = TerminalSpinner::new(format!(
        "Running tests for project in: {}",
        project.as_path().display()
    ));
    wait_with_stdout(cmd)?;
    drop(spinner);

    if !arguments.quiet {
        println!(
            "{}",
            format!(
                "Finished running tests for project in: {}",
                project.as_path().display()
            )
            .bold()
        );
        println!("Test results: {}", output_path.display());
    }

    Ok(())
}
