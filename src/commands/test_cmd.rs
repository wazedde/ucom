use std::process::{exit, Command};

use anyhow::anyhow;
use chrono::prelude::*;
use yansi::{Paint, Style};

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
    cmd.args(["-testPlatform", arguments.platform.as_ref()]);

    if let Some(target) = arguments.target {
        cmd.args(["-buildTarget", target.as_ref()]);
    } else {
        cmd.args([
            "-buildTarget",
            arguments.platform.as_build_target().as_ref(),
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

    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
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
            &format!(
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
        TermStat::println(
            "Finished",
            &format!(
                "{} tests for project in {}; total time {:.2}s",
                &arguments.platform,
                project.as_path().display(),
                time_delta_to_seconds(Utc::now().signed_duration_since(start_time))
            ),
            status,
        );

        let test_run = TestRun::from_file(&output_path)?;
        TermStat::println("Report", &output_path.to_string_lossy(), status);

        match arguments.show_results {
            ShowResults::Errors => {
                let r = test_run
                    .test_cases
                    .iter()
                    .filter(|tc| tc.result != TestResult::Passed);
                print_test_cases(r);
            }

            ShowResults::All => {
                print_test_cases(test_run.test_cases.iter());
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
            TermStat::stylize(status.as_ref(), status),
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

fn print_test_cases<'a>(test_cases: impl Iterator<Item = &'a TestCase>) {
    let mut test_cases = test_cases.peekable();
    if test_cases.peek().is_some() {
        println!();
    }

    for tc in test_cases {
        let (name_style, status) = if tc.result == TestResult::Passed {
            (Style::new(), Status::Ok)
        } else {
            (Style::new().red(), Status::Error)
        };

        println!(
            "{}: {}; finished in {:.2}s",
            TermStat::stylize(tc.result.as_ref(), status),
            tc.full_name.paint(name_style),
            tc.duration,
        );
    }
}
