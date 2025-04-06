use std::path::Path;
use std::process::{Command, exit};

use anyhow::anyhow;
use chrono::prelude::*;
use yansi::{Paint, Style};

use crate::cli_test::{ShowResults, TestArguments};
use crate::commands::TimeDeltaExt;
use crate::nunit::{TestCase, TestResult, TestRun};
use crate::unity::project::ProjectPath;
use crate::unity::{build_command_line, wait_with_stdout};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::{MessageType, StatusLine};

pub fn run_tests(arguments: &TestArguments) -> anyhow::Result<()> {
    let start_time = Utc::now();
    let project = ProjectPath::try_from(&arguments.project_dir)?;
    let project_unity_version = project.unity_version()?;
    let editor_exe = project_unity_version.editor_executable_path()?;
    project.ensure_assets_directory_exists()?;

    let test_results = format!(
        "tests-{}-{}.xml",
        arguments.platform,
        Utc::now().format("%Y%m%d%H%M%S")
    );

    let output_path = project.join(test_results);
    let test_command = arguments.build_cmd(&project, &editor_exe, &output_path);

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
                    &arguments.platform,
                    project.normalized_display()
                ),
            )
        };
        wait_with_stdout(test_command)
    };

    if let Err(e) = &tests_result {
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

    if !arguments.quiet {
        let status = match tests_result {
            Ok(()) => MessageType::Ok,
            Err(_) => MessageType::Error,
        };

        print_results(arguments, &start_time, &project, &output_path, status)?;
    }

    if tests_result.is_err() {
        // Unity returns exit code 2 when tests fail.
        exit(2);
    } else {
        Ok(())
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
            "{} tests for project in {}; total time {:.2}s",
            &arguments.platform,
            project.normalized_display(),
            Utc::now().signed_duration_since(start_time).as_seconds()
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
        "Result: {}. {}",
        MessageType::format_text(status.as_ref(), status),
        results,
    );
    Ok(())
}

impl TestArguments {
    fn build_cmd(&self, project: &ProjectPath, editor_exe: &Path, output_dir: &Path) -> Command {
        // Build the command to execute.
        let mut cmd = Command::new(editor_exe);
        cmd.args(["-projectPath", &project.to_string_lossy()]);
        cmd.arg("-runTests");
        cmd.args(["-testPlatform", self.platform.as_ref()]);

        if let Some(target) = self.target {
            cmd.args(["-buildTarget", target.as_ref()]);
        } else {
            cmd.args(["-buildTarget", self.platform.as_build_target().as_ref()]);
        }

        if self.no_batch_mode {
            cmd.arg("-batchmode");
        }

        if self.forget_project_path {
            cmd.arg("-forgetProjectPath");
        }

        if let Some(s) = &self.categories {
            cmd.args(["-testCategory", &format!("\"{s}\"")]);
        }

        if let Some(s) = &self.tests {
            cmd.args(["-testFilter", &format!("\"{s}\"")]);
        }

        if let Some(s) = &self.assemblies {
            cmd.args(["-assemblyNames", &format!("\"{s}\"")]);
        }

        cmd.args(["-testResults", &output_dir.to_string_lossy()]);

        if let Some(a) = self.args.as_ref() {
            cmd.args(a);
        }
        cmd
    }
}
fn print_test_cases<'a>(test_cases: impl Iterator<Item = &'a TestCase>) {
    let mut test_cases = test_cases.peekable();
    if test_cases.peek().is_some() {
        println!();
    }

    for test_case in test_cases {
        let (name_style, status) = if test_case.result == TestResult::Passed {
            (Style::new(), MessageType::Ok)
        } else {
            (Style::new().red(), MessageType::Error)
        };

        println!(
            "{}: {}; finished in {:.2}s",
            MessageType::format_text(test_case.result.as_ref(), status),
            test_case.full_name.paint(name_style),
            test_case.duration,
        );
    }
}
