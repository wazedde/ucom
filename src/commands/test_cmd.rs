use std::process::{exit, Command};

use anyhow::anyhow;
use chrono::prelude::*;
use colored::Colorize;

use crate::cli_test::{ShowResults, TestArguments};
use crate::commands::term_stat::{Status, TermStat};
use crate::commands::time_delta_to_seconds;
use crate::nunit::{TestCase, TestResult, TestRun};
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

    if let Some(target) = arguments.target {
        cmd.args(["-buildTarget", &target.to_string()]);
    } else {
        cmd.args([
            "-buildTarget",
            &arguments.platform.as_build_target().to_string(),
        ]);
    }

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
            format!(
                "{} tests for project in {}",
                &arguments.platform,
                project.as_path().display()
            ),
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
        Ok(()) => Status::Ok,
        Err(_) => Status::Error,
    };

    if !arguments.quiet {
        TermStat::println_stat(
            "Finished",
            format!(
                "{} tests for project in {}; total time {:.2}s",
                &arguments.platform,
                project.as_path().display(),
                time_delta_to_seconds(Utc::now().signed_duration_since(start_time))
            ),
            status,
        );

        let test_run = TestRun::from_file(&output_path)?;
        TermStat::println_stat("Report", output_path.to_string_lossy(), status);

        match arguments.show_results {
            ShowResults::Errors => {
                let r = test_run
                    .test_cases
                    .iter()
                    .filter(|tc| tc.result != TestResult::Passed);
                print_results(r);
            }

            ShowResults::All => {
                print_results(test_run.test_cases.iter());
            }
            _ => {}
        };

        println!();
        let results = format!(
            "{} total; {} passed; {} failed; {} inconclusive; {} skipped; {} asserts; finished in {:.2}s",
            test_run.stats.total,
            test_run.stats.passed,
            test_run.stats.failed,
            test_run.stats.inconclusive,
            test_run.stats.skipped,
            test_run.stats.asserts,
            test_run.stats.duration,
        );

        println!(
            "Result: {}. {}",
            TermStat::get_colored(status.to_string(), status),
            results,
        );
    }

    if result.is_err() {
        // Unity returns exit code 2 when tests fail.
        exit(2);
    } else {
        Ok(())
    }
}

fn print_results<'a>(filtered: impl Iterator<Item = &'a TestCase>) {
    let mut filtered = filtered.peekable();
    if filtered.peek().is_some() {
        println!();
    }

    for tc in filtered {
        if tc.result == TestResult::Passed {
            println!(
                "{}: {}; finished in {:.2}s",
                TermStat::get_colored(tc.result.to_string(), Status::Ok),
                tc.full_name,
                tc.duration,
            );
        } else {
            println!(
                "{}: {}; finished in {:.2}s",
                TermStat::get_colored(tc.result.to_string(), Status::Error),
                tc.full_name.red(),
                tc.duration,
            );
        };
    }
}
