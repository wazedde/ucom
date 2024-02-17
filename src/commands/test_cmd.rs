use std::process::Command;

use chrono::prelude::*;

use crate::cli::TestArguments;
use crate::commands::term_stat::TermStat;
use crate::commands::time_delta_to_seconds;
use crate::unity::{build_command_line, wait_with_stdout, ProjectPath};

pub fn run_tests(arguments: TestArguments) -> anyhow::Result<()> {
    let start_time = Utc::now();
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

    let ts = TermStat::new_quiet(
        arguments.quiet,
        "Running",
        format!("tests for project in {}", project.as_path().display()),
    );

    wait_with_stdout(cmd)?;

    drop(ts);

    if !arguments.quiet {
        TermStat::print_stat_ok(
            "Running",
            format!("tests for project in {}", project.as_path().display()),
        );

        TermStat::print_stat_ok(
            "Finished",
            format!(
                "in {:.2}s",
                time_delta_to_seconds(Utc::now().signed_duration_since(start_time))
            ),
        );

        TermStat::print_stat_ok("Results", output_path.to_string_lossy());
    }

    Ok(())
}
