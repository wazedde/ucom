use crate::cli::{CacheAction, Cli, Command};
use crate::commands::test_cmd::run_tests;
use crate::commands::{
    INDENT, add_to_project, build_project, find_project_updates, install_latest_matching,
    list_versions, new_project, open_project, project_info, run_unity,
};
use crate::style_definitions::STYLE_ERROR;
use crate::unity::release_api::UpdatePolicy;
use anyhow::Context;
use clap::Parser;
use std::io::IsTerminal;
use std::process::ExitCode;
use utils::content_cache::{delete_cache_directory, ucom_cache_dir};
use utils::{ReportedFailure, cancel};
use yansi::Paint;

mod cli;
mod cli_add;
mod cli_build;
mod cli_new;
mod cli_run;
mod cli_test;
mod commands;
mod nunit;
mod style_definitions;
mod unity;
mod utils;

fn main() -> ExitCode {
    // Commands return their errors instead of exiting, so destructors (status lines, progress
    // indicators, temporary files) have run by the time the exit code is chosen here.
    let cli = Cli::parse();
    cancel::install_handler();

    // Errors go to stderr, so whether they are coloured depends on stderr, not stdout.
    let colour = colour_allowed(cli.no_color);
    let colour_errors = colour && std::io::stderr().is_terminal();
    if !colour || !std::io::stdout().is_terminal() {
        yansi::disable();
    }

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => match ReportedFailure::find(&e) {
            Some(&ReportedFailure::Silent { exit_code, .. }) => ExitCode::from(exit_code),
            Some(failure @ ReportedFailure::Interrupted) => {
                eprintln!("{failure}");
                // Everything has been dropped; end the way a shell expects after Ctrl+C.
                cancel::exit_interrupted()
            }
            None => {
                // `DEFAULT`, not `enable()`: on Windows it first switches the console to ANSI
                // mode, and leaves colour off if that fails.
                yansi::whenever(if colour_errors {
                    yansi::Condition::DEFAULT
                } else {
                    yansi::Condition::NEVER
                });
                eprintln!(
                    "{}",
                    format_error(&e, |headline| headline.paint(STYLE_ERROR).to_string())
                );
                ExitCode::FAILURE
            }
        },
    }
}

/// Whether colour may be used at all: not with `--no-color`, nor when the `NO_COLOR`
/// environment variable is set to a non-empty value (<https://no-color.org>).
fn colour_allowed(no_color_flag: bool) -> bool {
    colour_allowed_by(no_color_flag, std::env::var_os("NO_COLOR").as_deref())
}

fn colour_allowed_by(no_color_flag: bool, no_color_env: Option<&std::ffi::OsStr>) -> bool {
    !no_color_flag && no_color_env.is_none_or(|v| v.is_empty())
}

/// Formats an error as returning it from `main` would (`Error: ...`, then `Caused by:` and the
/// causes), with the headline passed through `style`.
fn format_error(error: &anyhow::Error, style: impl Fn(&str) -> String) -> String {
    let headline = error.to_string();
    let full = format!("{error:?}");
    // The debug output starts with the headline; only that part is styled. Should that ever
    // change, the unstyled output still shows every cause.
    match full.strip_prefix(&headline) {
        Some(rest) => format!("Error: {}{rest}", style(&headline)),
        None => format!("Error: {full}"),
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let Some(command) = cli.command else {
        return Ok(());
    };

    match command {
        Command::List {
            list_type,
            version_filter,
            force,
        } => {
            let mode = if force {
                UpdatePolicy::ForceRefresh
            } else {
                UpdatePolicy::Incremental
            };
            list_versions(list_type, version_filter.as_ref(), mode)
                .with_context(|| format!("Cannot list `{list_type}`"))
        }

        Command::Install { version } => {
            install_latest_matching(&version, UpdatePolicy::Incremental)
                .context("Cannot install the Unity version")
        }

        Command::Info {
            project_dir,
            install_required,
            recursive,
            packages,
            report,
        } => project_info(
            &project_dir,
            packages,
            install_required,
            recursive,
            report,
            UpdatePolicy::Incremental,
        )
        .context("Cannot show project info"),

        Command::Updates {
            project_dir,
            install_latest,
            report,
        } => find_project_updates(
            &project_dir,
            install_latest,
            report,
            UpdatePolicy::Incremental,
        )
        .context("Cannot show Unity updates for the project"),

        Command::Run(settings) => run_unity(settings).context("Cannot run Unity"),

        Command::New(settings) => {
            new_project(settings).context("Cannot create the new Unity project")
        }

        Command::Open(settings) => open_project(settings).context("Cannot open the Unity project"),

        Command::Build(settings) => build_project(&settings).context("Cannot build the project"),

        Command::Test(settings) => run_tests(&settings).context("Cannot run tests"),

        Command::Add(arguments) => {
            add_to_project(&arguments).context("Cannot add the file to the project")
        }

        Command::Cache { action: command } => {
            match command {
                CacheAction::Clear => {
                    delete_cache_directory();
                    println!("Cleared cache at: {}", ucom_cache_dir()?.display());
                }
                CacheAction::List => {
                    let cache_dir = ucom_cache_dir()?;
                    if !cache_dir.exists() {
                        println!("No cache found at: {}", cache_dir.display());
                        return Ok(());
                    }

                    println!("Cached files at: {}", cache_dir.display());
                    for file in ucom_cache_dir()?.read_dir()? {
                        println!("{}{}", INDENT, file?.file_name().to_string_lossy());
                    }
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_error_headline_is_styled() {
        let error = anyhow::anyhow!("Path does not exist").context("Cannot show project info");

        let text = format_error(&error, |h| format!("<{h}>"));

        assert_eq!(
            text,
            "Error: <Cannot show project info>\n\nCaused by:\n    Path does not exist"
        );
    }

    #[test]
    fn no_color_turns_colour_off_unless_empty() {
        use std::ffi::OsStr;

        assert!(colour_allowed_by(false, None));
        assert!(!colour_allowed_by(false, Some(OsStr::new("1"))));
        // An empty value does not count, as https://no-color.org specifies.
        assert!(colour_allowed_by(false, Some(OsStr::new(""))));
        assert!(!colour_allowed_by(true, None));
    }

    #[test]
    fn an_error_without_causes_is_one_line() {
        let error = anyhow::anyhow!("Something went wrong");
        assert_eq!(
            format_error(&error, |h| format!("<{h}>")),
            "Error: <Something went wrong>"
        );
    }
}
