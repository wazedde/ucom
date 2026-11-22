mod build_log;
mod editor;
mod plan;
mod script;

use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::cli_build::BuildArguments;
use crate::commands::builder_script::{ensure_builder_supports, warn_if_builder_outdated};
use crate::commands::project_setup::ProjectSetup;
use crate::commands::release_issue::report_version_issues;
use crate::style_definitions::Tone;
use crate::unity::find_unity_editor_process;
use crate::utils::native_progress::{NativeProgressBar, NativeProgressState};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::spawn_cmd::{
    CommandError, OnCancel, build_command_line, wait_with_log_output, wait_with_stdout,
};
use crate::utils::status_line::{StatusLine, print_status};
use crate::utils::warn_non_fatal;

use build_log::{log_errors, print_build_report, remove_old_log_file};
use editor::build_via_editor;
use plan::BuildPlan;
use script::inject_build_script;

/// Runs the build command.
pub fn build_project(arguments: &BuildArguments) -> anyhow::Result<()> {
    let start_time = Instant::now();
    let setup = ProjectSetup::new(&arguments.project_dir)?;
    let plan = BuildPlan::new(arguments, &setup.project)?;

    let editor_pid = find_unity_editor_process(&setup.project);

    // A dry run still shows what would be run.
    if editor_pid.is_some() || plan.batch_uses_builder() {
        match ensure_builder_supports(setup.unity_version) {
            Err(e) if plan.args.dry_run => {
                warn_non_fatal!("{e}");
            }
            result => result?,
        }
        warn_if_builder_outdated(&setup.project);
    }

    // If the project is open in the Unity editor, build there via IPC.
    if let Some(editor_pid) = editor_pid {
        return build_via_editor(&plan, &setup, editor_pid, start_time);
    }

    // Otherwise, build in a new Unity process.
    let editor_path = setup.editor_executable()?;
    let build_command = plan.batch_command(&setup.project, &editor_path);

    if plan.args.dry_run {
        println!("{}", build_command_line(&build_command));
        return Ok(());
    }
    let build_status = start_build_status("Building", &plan, &setup);

    let _progress_guard = NativeProgressBar::with_state(NativeProgressState::Indeterminate);

    // Removes a temporary build script when dropped, including on Ctrl+C and early returns.
    let injected_script = inject_build_script(&setup.project, plan.args.inject)?;

    remove_old_log_file(&plan.log_path)?;

    let build_result = if plan.show_log() {
        wait_with_log_output(build_command, &plan.log_path)
    } else {
        wait_with_stdout(build_command, OnCancel::Kill)
    };

    drop(injected_script);
    drop(build_status);

    if let Err(e @ CommandError::Cancelled) = build_result {
        return Err(e.into());
    }

    finish_build(&plan, &setup, start_time, build_result.is_ok(), |status| {
        print_build_report(&plan.log_path, status);
    });
    build_result.map_err(|e| match e {
        // Unity ran and failed; its log says why, unless it failed before logging an error.
        // Then its exit code and stderr are all there is.
        CommandError::Failed { .. } => log_errors(&plan.log_path).unwrap_or_else(|| e.into()),
        // Unity could not be started or waited for, so its log has nothing to say.
        _ => e.into(),
    })
}

/// Describes a build for the lines that announce it and report its outcome.
fn build_description(setup: &ProjectSetup, target: impl Display) -> String {
    format!(
        "Unity {} {target} project in {}",
        setup.unity_version,
        setup.project.normalized_display()
    )
}

/// Reports a finished build, in the same order for batch and editor builds: the outcome, the
/// build report (printed by `print_report`), the total time and known issues of the Unity
/// version. Then, if the build succeeded, removes the folders `--clean` asks for.
fn finish_build(
    plan: &BuildPlan,
    setup: &ProjectSetup,
    start_time: Instant,
    succeeded: bool,
    print_report: impl FnOnce(Tone),
) {
    let (label, tone) = if succeeded {
        ("Succeeded", Tone::Success)
    } else {
        ("Failed", Tone::Error)
    };
    print_status(
        tone,
        label,
        format_args!("building {}", build_description(setup, plan.args.target)),
    );
    print_report(tone);
    print_status(
        tone,
        "Total time",
        format_args!("{t:.2}s", t = start_time.elapsed().as_secs_f64()),
    );
    report_version_issues(setup.unity_version);

    if succeeded && plan.args.clean {
        clean_output_directory(&plan.output_path);
    }
}

/// Announces the start of a build: a status line in quiet mode, otherwise a printed message.
/// The status line is cleared when the returned value is dropped.
fn start_build_status(label: &str, plan: &BuildPlan, setup: &ProjectSetup) -> StatusLine {
    let build_text = build_description(setup, plan.args.target);

    if !plan.args.quiet {
        print_status(Tone::Info, label, &build_text);
    }
    StatusLine::new_if(plan.args.quiet, label, &build_text)
}

/// Name endings of the folders Unity writes next to a player that must not be shipped.
const DO_NOT_SHIP_SUFFIXES: [&str; 2] = [
    "_BurstDebugInformation_DoNotShip",
    "_BackUpThisFolder_ButDontShipItWithYourGame",
];

/// Removes the folders Unity writes next to a player that must not be shipped.
///
/// Runs after a successful build has been reported, so a folder that cannot be removed is only
/// a warning.
fn clean_output_directory(path: &Path) {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            warn_non_fatal!(
                "Could not clean the output directory {}: {e}",
                path.normalized_display()
            );
            return;
        }
    };

    let to_delete = entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| DO_NOT_SHIP_SUFFIXES.iter().any(|s| name.ends_with(s)))
        })
        .map(|entry| entry.path())
        .filter(|p| p.is_dir());

    for dir in to_delete {
        println!("Removing directory: {}", dir.normalized_display());
        if let Err(e) = fs::remove_dir_all(&dir) {
            warn_non_fatal!(
                "Could not remove directory {}: {e}",
                dir.normalized_display()
            );
        }
    }
}
