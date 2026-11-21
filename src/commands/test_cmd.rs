use std::path::Path;
use std::process::Command;
use std::time::Instant;

use anyhow::bail;
use chrono::Local;
use uuid::Uuid;

use crate::cli_test::{ShowResults, TestArguments};
use crate::commands::output::INDENT;
use crate::commands::project_setup::ProjectSetup;
use crate::commands::unity_command_builder::UnityCommandBuilder;
use crate::nunit::{TestCase, TestResult, TestRun};
use crate::style_definitions::{Styled, Tone};
use crate::unity::ProjectPath;
use crate::utils::ReportedFailure;
use crate::utils::native_progress::{NativeProgressBar, NativeProgressState};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::spawn_cmd::{OnCancel, build_command_line, wait_with_stdout};
use crate::utils::status_line::{StatusLine, print_status};

pub fn run_tests(arguments: &TestArguments) -> anyhow::Result<()> {
    let start_time = Instant::now();
    let setup = ProjectSetup::new(&arguments.project_dir)?;
    let editor_exe = setup.editor_executable()?;
    setup.project.ensure_assets_directory_exists()?;

    // The random suffix keeps two runs started in the same second apart, and makes sure the
    // file checked for after the run is this run's.
    let test_results = format!(
        "tests-{}-{}-{}.xml",
        arguments.platform,
        Local::now().format("%Y%m%d%H%M%S"),
        &Uuid::new_v4().simple().to_string()[..8]
    );

    let output_path = setup.project.join(test_results);
    let test_command = arguments.build_cmd(&setup.project, &editor_exe, &output_path);

    if arguments.dry_run {
        println!("{}", build_command_line(&test_command));
        return Ok(());
    }

    let tests_result = {
        let _status = StatusLine::new_if(
            !arguments.quiet,
            "Running",
            format!(
                "{} tests for project in {}",
                arguments.platform,
                setup.project.normalized_display()
            ),
        );
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
        print_results(
            arguments,
            start_time,
            &setup.project,
            &output_path,
            tests_failed,
        )?;
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
    start_time: Instant,
    project: &ProjectPath,
    output_path: &Path,
    tests_failed: bool,
) -> anyhow::Result<()> {
    let (result, tone) = if tests_failed {
        ("Error", Tone::Error)
    } else {
        ("Ok", Tone::Success)
    };

    print_status(
        tone,
        "Finished",
        format_args!(
            "{p} tests for project in {d}; total time {t:.2}s",
            p = arguments.platform,
            d = project.normalized_display(),
            t = start_time.elapsed().as_secs_f64()
        ),
    );

    let test_run = TestRun::from_file(output_path)?;
    print_status(tone, "Report", output_path.to_string_lossy());

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
        s = result.tone(tone).bold(),
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
            builder = builder.with_build_target(target);
        } else {
            builder = builder.with_build_target(self.platform.as_build_target());
        }

        // Add forget project path flag if needed
        if self.forget_project_path {
            builder = builder.add_arg("-forgetProjectPath");
        }

        // Add test filters. Each value is passed as one argument without a shell, so it must not
        // be quoted: Unity would take the quotes as part of the filter and match nothing.
        if let Some(s) = &self.categories {
            builder = builder.add_arg("-testCategory").add_arg(s);
        }

        if let Some(s) = &self.tests {
            builder = builder.add_arg("-testFilter").add_arg(s);
        }

        if let Some(s) = &self.assemblies {
            builder = builder.add_arg("-assemblyNames").add_arg(s);
        }

        // Add test results path
        builder = builder.add_arg("-testResults").add_arg(output_dir);

        // Add any additional arguments
        if let Some(a) = self.args.as_ref() {
            builder = builder.add_args(a);
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
        let (name_tone, tone) = if test_case.result == TestResult::Passed {
            (Tone::Plain, Tone::Success)
        } else {
            (Tone::Error, Tone::Error)
        };

        println!(
            "{s}: {n}; finished in {t:.2}s",
            s = test_case.result.as_ref().tone(tone).bold(),
            n = test_case.full_name.tone(name_tone),
            t = test_case.duration,
        );

        if let Some(message) = result_message(test_case) {
            for line in message.lines() {
                println!("{INDENT}{line}");
            }
        }
    }
}

/// The message that explains a result other than passed: the failure message, or why the
/// test was skipped.
fn result_message(test_case: &TestCase) -> Option<&str> {
    [&test_case.failure_message, &test_case.skip_reason]
        .into_iter()
        .map(|m| m.trim())
        .find(|m| !m.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command as CliCommand};
    use crate::utils::test_util::temp_project;
    use clap::Parser;

    /// The value after `flag` in the Unity command line for `ucom test editmode <extra>`.
    fn unity_arg_after(extra: &[&str], flag: &str) -> Option<String> {
        let (_dir, project) = temp_project("test-args");
        let cli = Cli::try_parse_from(["ucom", "test", "editmode"].iter().chain(extra)).unwrap();
        let Some(CliCommand::Test(arguments)) = cli.command else {
            unreachable!("parsed a test command")
        };
        let cmd = arguments.build_cmd(&project, Path::new("Unity"), Path::new("results.xml"));
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let i = args.iter().position(|a| a == flag)?;
        args.get(i + 1).cloned()
    }

    #[test]
    fn test_filters_are_passed_to_unity_without_quotes() {
        let after = |option: &str, flag| unity_arg_after(&[option, "!Slow;UI"], flag);
        assert_eq!(
            after("--categories", "-testCategory").as_deref(),
            Some("!Slow;UI")
        );
        assert_eq!(after("--tests", "-testFilter").as_deref(), Some("!Slow;UI"));
        assert_eq!(
            after("--assemblies", "-assemblyNames").as_deref(),
            Some("!Slow;UI")
        );
    }

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
