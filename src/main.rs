use crate::cli::{Cli, Command};
use crate::commands::{
    add_to_project, build_project, find_project_updates, install_latest_matching, list_versions,
    manage_cache, new_project, open_project, project_info, run_tests, run_unity,
};
use crate::style_definitions::{Styled, Tone};
use crate::unity::release_api::UpdatePolicy;
use anyhow::Context;
use clap::{ColorChoice, CommandFactory, FromArgMatches};
use std::io::IsTerminal;
use std::process::ExitCode;
use utils::{ReportedFailure, cancel};

mod cli;
mod cli_add;
mod cli_build;
mod cli_cache;
mod cli_info;
mod cli_list;
mod cli_new;
mod cli_open;
mod cli_run;
mod cli_test;
mod cli_updates;
mod commands;
mod nunit;
mod style_definitions;
mod unity;
mod utils;

fn main() -> ExitCode {
    // Commands return their errors instead of exiting, so destructors (status lines, progress
    // indicators, temporary files) have run by the time the exit code is chosen here.
    let cli = parse_cli();
    cancel::install_handler();

    // Errors go to stderr, so whether they are coloured depends on stderr, not stdout.
    let colour = colour_allowed(cli.no_color);
    let forced = colour_forced();
    yansi::whenever(colour_condition(
        colour,
        forced,
        std::io::stdout().is_terminal(),
    ));
    let error_colour = colour_condition(colour, forced, std::io::stderr().is_terminal());

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
                yansi::whenever(error_colour);
                eprintln!(
                    "{}",
                    format_error(&e, |headline| headline.tone(Tone::Error).to_string())
                );
                ExitCode::FAILURE
            }
        },
    }
}

/// Parses the command line as `Cli::parse` does, but without colour in clap's own help and error
/// messages when `--no-color` is given.
///
/// Clap follows `NO_COLOR` and `CLICOLOR_FORCE` as ucom does. It also turns colour off for
/// `CLICOLOR=0` and `TERM=dumb`, which ucom's own output does not.
fn parse_cli() -> Cli {
    let mut command = Cli::command();
    if no_color_flag_in(std::env::args_os().skip(1)) {
        command = command.color(ColorChoice::Never);
    }
    Cli::from_arg_matches(&command.get_matches()).unwrap_or_else(|e| e.exit())
}

/// Whether the arguments, without the program name, hold `--no-color` or `-n`. It is a
/// top-level flag, so only the flags in front of the subcommand count.
fn no_color_flag_in(args: impl IntoIterator<Item = std::ffi::OsString>) -> bool {
    args.into_iter()
        .map_while(|arg| arg.into_string().ok().filter(|arg| arg.starts_with('-')))
        .any(|arg| arg == "--no-color" || (!arg.starts_with("--") && arg[1..].contains('n')))
}

/// When yansi may colour a stream: never without colour, always when it is forced, otherwise only
/// on a terminal. A terminal gets `DEFAULT`, not `ALWAYS`: on Windows it first switches the
/// console to ANSI mode and leaves colour off if that fails. Forced colour gets `ALWAYS`, since
/// a pipe or file has no console mode to switch.
fn colour_condition(allowed: bool, forced: bool, is_terminal: bool) -> yansi::Condition {
    match (allowed, forced, is_terminal) {
        (false, _, _) => yansi::Condition::NEVER,
        (true, true, _) => yansi::Condition::ALWAYS,
        (true, false, true) => yansi::Condition::DEFAULT,
        (true, false, false) => yansi::Condition::NEVER,
    }
}

/// Whether colour is forced even when the output is not a terminal: the `CLICOLOR_FORCE`
/// environment variable is set to a non-empty value (<https://bixense.com/clicolors/>), the
/// same rule clap uses. `--no-color` and `NO_COLOR` still turn colour off.
fn colour_forced() -> bool {
    std::env::var_os("CLICOLOR_FORCE").is_some_and(|value| !value.is_empty())
}

/// Whether colour may be used at all: not with `--no-color`, nor when the `NO_COLOR`
/// environment variable is set to a non-empty value (<https://no-color.org>).
fn colour_allowed(no_color_flag: bool) -> bool {
    colour_allowed_by(no_color_flag, std::env::var_os("NO_COLOR").as_deref())
}

fn colour_allowed_by(no_color_flag: bool, no_color_env: Option<&std::ffi::OsStr>) -> bool {
    !no_color_flag && no_color_env.is_none_or(std::ffi::OsStr::is_empty)
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
        Command::List(arguments) => {
            let mode = if arguments.force {
                UpdatePolicy::ForceRefresh
            } else {
                UpdatePolicy::Incremental
            };
            list_versions(&arguments, mode)
                .with_context(|| format!("Cannot list `{}`", arguments.list_type))
        }

        Command::Install { version } => {
            install_latest_matching(&version, UpdatePolicy::Incremental)
                .context("Cannot install the Unity version")
        }

        Command::Info(arguments) => {
            project_info(&arguments, UpdatePolicy::Incremental).context("Cannot show project info")
        }

        Command::Updates(arguments) => find_project_updates(&arguments, UpdatePolicy::Incremental)
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

        Command::Cache(arguments) => manage_cache(&arguments),
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

    /// `no_color_flag_in` assumes the only top-level short flags are `-n`, `-h` and `-V`, and
    /// that no top-level argument takes a value. A new one would need a change there.
    #[test]
    fn no_color_flag_scan_matches_the_top_level_arguments() {
        let mut command = Cli::command();
        command.build();
        for arg in command.get_arguments() {
            let id = arg.get_id();
            assert!(
                matches!(arg.get_short(), None | Some('n' | 'h' | 'V')),
                "update no_color_flag_in for the short flag of `{id}`"
            );
            assert!(
                !arg.get_action().takes_values(),
                "update no_color_flag_in for `{id}`, which takes a value"
            );
        }
    }

    #[test]
    fn no_color_flag_counts_only_in_front_of_the_subcommand() {
        let found = |args: &[&str]| no_color_flag_in(args.iter().map(std::ffi::OsString::from));

        assert!(found(&["--no-color", "list"]));
        assert!(found(&["-n", "list"]));
        assert!(found(&["-nh"]));
        assert!(!found(&["list"]));
        // After the subcommand, `-n` and `--no-color` belong to it or to Unity.
        assert!(!found(&["new", "-n", "MyProject"]));
        assert!(!found(&["run", "--", "--no-color"]));
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
