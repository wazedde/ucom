use std::process::{exit, Command};

use anyhow::anyhow;
use chrono::prelude::*;

use nunit::TestResult;

use crate::cli::TestArguments;
use crate::commands::term_stat::{Status, TermStat};
use crate::commands::time_delta_to_seconds;
use crate::nunit;
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

    cmd.args([
        "-buildTarget",
        &arguments.platform.as_build_target().to_string(),
    ]);

    if !arguments.no_batch_mode {
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

    let ts = if arguments.quiet {
        TermStat::new_inactive()
    } else {
        TermStat::new(
            "Running",
            format!("tests for project in {}", project.as_path().display()),
        )
    };

    let result = wait_with_stdout(cmd);

    drop(ts);

    if let Err(e) = &result {
        // If the error was not caused by the command exiting with code 2 (tests failed), return it.
        if e.exit_code != 2 {
            return Err(anyhow!("{e}"));
        }
    }

    if !output_path.exists() {
        // Stupid workaround for Unity not returning an error when project is already open.
        return Err(anyhow!(
            "Unable to run tests, is another Unity instance running with this same project open?"
        ));
    }

    let status = match result {
        Ok(_) => Status::Ok,
        Err(_) => Status::Error,
    };

    if !arguments.quiet {
        TermStat::println_stat(
            "Running",
            format!("tests for project in {}", project.as_path().display()),
            status,
        );

        let stats = nunit::read_stats_from_file(&output_path)?;

        let status1 = if stats.result == TestResult::Passed {
            Status::Ok
        } else {
            Status::Error
        };

        TermStat::println_stat("Result", stats.result.to_string(), status1);

        TermStat::println_stat(
            "Finished",
            format!(
                "in {:.2}s",
                time_delta_to_seconds(Utc::now().signed_duration_since(start_time))
            ),
            status,
        );

        let results = format!(
            "Total: {}, Passed: {}, Failed: {}, Inconclusive: {}, Skipped: {}, Asserts: {}",
            stats.total,
            stats.passed,
            stats.failed,
            stats.inconclusive,
            stats.skipped,
            stats.asserts
        );

        TermStat::println_stat("Totals", results, status1);

        TermStat::println_stat("Report", output_path.to_string_lossy(), status);
    }

    if result.is_err() {
        exit(2);
    }

    Ok(())
}
