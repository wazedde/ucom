use std::path::Path;
use std::process::Command;

use anyhow::bail;
use chrono::prelude::*;
use yansi::Paint;

use crate::cli_test::{ShowResults, TestArguments};
use crate::commands::unity_command_builder::UnityCommandBuilder;
use crate::commands::{ProjectSetup, TimeDeltaExt};
use crate::nunit::{TestCase, TestResult, TestRun};
use crate::style_definitions::{STYLE_ERROR, STYLE_PLAIN};
use crate::unity::{OnCancel, ProjectPath, build_command_line, wait_with_stdout};
use crate::utils::ReportedFailure;
use crate::utils::native_progress::{NativeProgressBar, NativeProgressState};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::{MessageType, StatusLine};

pub fn run_tests(arguments: &TestArguments) -> anyhow::Result<()> {
    let start_time = Utc::now();
    let setup = ProjectSetup::new(&arguments.project_dir)?;
    let editor_exe = setup.editor_executable()?;
    setup.project.ensure_assets_directory_exists()?;

    let test_results = format!(
        "tests-{}-{}.xml",
        arguments.platform,
        Local::now().format("%Y%m%d%H%M%S")
    );

    let output_path = setup.project.join(test_results);
    let test_command = arguments.build_cmd(&setup.project, &editor_exe, &output_path);

    if arguments.dry_run {
        println!("{}", build_command_line(&test_command));
        return Ok(());
    }

    let tests_result = {
        let _status = if arguments.quiet {
            StatusLine::new_silent()
        } else {
            StatusLine::new(
                "Running",
                format!(
                    "{} tests for project in {}",
                    arguments.platform,
                    setup.project.normalized_display()
                ),
            )
        };
        let _progress_guard = NativeProgressBar::with_state(NativeProgressState::Indeterminate);
        wait_with_stdout(test_command, OnCancel::Kill)
    };

    let tests_failed = match tests_result {
        Ok(()) => false,
        Err(e) if e.exit_code() == Some(i32::from(TESTS_FAILED_EXIT_CODE)) => true,
        // Returned with its type, so that `main` recognises a cancellation.
        Err(e) => return Err(e.into()),
    };

    if !output_path.exists() {
        // Stupid workaround for Unity not returning an error when project is already open.
        bail!(
            "Unable to run tests, is another Unity instance running with this same project open?"
        );
    }

    if !arguments.quiet {
        let status = if tests_failed {
            MessageType::Error
        } else {
            MessageType::Ok
        };

        print_results(arguments, &start_time, &setup.project, &output_path, status)?;
    }

    if tests_failed {
        return Err(tests_failed_error().into());
    }
    Ok(())
}

/// Unity's exit code when tests fail, which ucom passes on.
const TESTS_FAILED_EXIT_CODE: u8 = 2;

/// The error for a test run in which tests failed. The results have already been reported.
fn tests_failed_error() -> ReportedFailure {
    ReportedFailure::Silent {
        exit_code: TESTS_FAILED_EXIT_CODE,
        description: "One or more tests failed",
    }
}

fn print_results(
    arguments: &TestArguments,
    start_time: &DateTime<Utc>,
    project: &ProjectPath,
    output_path: &Path,
    status: MessageType,
) -> anyhow::Result<()> {
    MessageType::print_line(
        "Finished",
        format!(
            "{p} tests for project in {d}; total time {t:.2}s",
            p = arguments.platform,
            d = project.normalized_display(),
            t = Utc::now().signed_duration_since(start_time).as_seconds()
        ),
        status,
    );

    let test_run = TestRun::from_file(output_path)?;
    MessageType::print_line("Report", output_path.to_string_lossy(), status);

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
        ShowResults::None => {}
    }

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
        "Result: {s}. {r}",
        s = MessageType::format_text(status.as_ref(), status),
        r = results,
    );
    Ok(())
}

impl TestArguments {
    fn build_cmd(&self, project: &ProjectPath, editor_exe: &Path, output_dir: &Path) -> Command {
        // Build the command using the builder pattern.
        let mut builder = UnityCommandBuilder::new(editor_exe.to_path_buf())
            .with_project_path(project.to_path_buf())
            .add_arg("-runTests")
            .add_arg("-testPlatform")
            .add_arg(self.platform.as_ref())
            .batch_mode(self.no_batch_mode);

        // Set build target
        if let Some(target) = self.target {
            builder = builder.with_build_target(target.as_ref());
        } else {
            builder = builder.with_build_target(self.platform.as_build_target().as_ref());
        }

        // Add forget project path flag if needed
        if self.forget_project_path {
            builder = builder.add_arg("-forgetProjectPath");
        }

        // Add test filters
        if let Some(s) = &self.categories {
            builder = builder.add_arg("-testCategory").add_arg(format!("\"{s}\""));
        }

        if let Some(s) = &self.tests {
            builder = builder.add_arg("-testFilter").add_arg(format!("\"{s}\""));
        }

        if let Some(s) = &self.assemblies {
            builder = builder
                .add_arg("-assemblyNames")
                .add_arg(format!("\"{s}\""));
        }

        // Add test results path
        builder = builder
            .add_arg("-testResults")
            .add_arg(output_dir.to_string_lossy().to_string());

        // Add any additional arguments
        if let Some(a) = self.args.as_ref() {
            builder = builder.add_args(a.iter().cloned());
        }

        builder.build()
    }
}

fn print_test_cases<'a>(test_cases: impl Iterator<Item = &'a TestCase>) {
    let mut test_cases = test_cases.peekable();
    if test_cases.peek().is_some() {
        println!();
    }

    for test_case in test_cases {
        let (name_style, status) = if test_case.result == TestResult::Passed {
            (STYLE_PLAIN, MessageType::Ok)
        } else {
            (STYLE_ERROR, MessageType::Error)
        };

        println!(
            "{s}: {n}; finished in {t:.2}s",
            s = MessageType::format_text(test_case.result.as_ref(), status),
            n = test_case.full_name.paint(name_style),
            t = test_case.duration,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_tests_are_reported_with_unitys_exit_code() {
        let error = anyhow::Error::from(tests_failed_error()).context("Cannot run tests");
        // Exit code 2 is part of ucom's command line contract, so it is spelled out here.
        assert!(matches!(
            ReportedFailure::find(&error),
            Some(ReportedFailure::Silent { exit_code: 2, .. })
        ));
    }
}
