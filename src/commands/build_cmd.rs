mod build_log;
mod editor;
mod plan;
mod script;

use std::fs;
use std::path::Path;

use crate::cli_build::BuildArguments;
use crate::commands::{
    ProjectSetup, TimeDeltaExt, ensure_builder_supports, report_version_issues, warn_non_fatal,
};
use crate::unity::{
    CommandError, OnCancel, build_command_line, find_unity_editor_process, wait_with_log_output,
    wait_with_stdout,
};
use crate::utils::native_progress::{NativeProgressBar, NativeProgressState};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::{MessageType, StatusLine};
use anyhow::Context;
use chrono::Utc;

use build_log::{collect_log_errors, print_build_report, remove_old_log_file};
use editor::build_via_editor;
use plan::BuildPlan;
use script::{inject_build_script, warn_if_builder_outdated};

/// Runs the build command.
pub fn build_project(arguments: &BuildArguments) -> anyhow::Result<()> {
    let start_time = Utc::now();
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

    let (build_status, log_tag) = if build_result.is_ok() {
        if plan.args.clean {
            clean_output_directory(&plan.output_path)?;
        }
        (MessageType::Ok, "Succeeded")
    } else {
        (MessageType::Error, "Failed")
    };

    MessageType::print_line(
        log_tag,
        format!(
            "building Unity {} {} project in {}",
            setup.unity_version,
            plan.args.target,
            setup.project.normalized_display()
        ),
        build_status,
    );

    MessageType::print_line(
        "Total time",
        format!(
            "{t:.2}s",
            t = Utc::now().signed_duration_since(start_time).as_seconds()
        ),
        build_status,
    );

    print_build_report(&plan.log_path, build_status);
    report_version_issues(setup.unity_version);
    build_result.map_err(|_| collect_log_errors(&plan.log_path))
}

/// Announces the start of a build: a status line in quiet mode, otherwise a printed message.
/// The status line is cleared when the returned value is dropped.
fn start_build_status(label: &str, plan: &BuildPlan, setup: &ProjectSetup) -> StatusLine {
    let build_text = format!(
        "Unity {} {} project in {}",
        setup.unity_version,
        plan.args.target,
        setup.project.normalized_display()
    );

    if plan.args.quiet {
        StatusLine::new(label, &build_text)
    } else {
        MessageType::print_line(label, &build_text, MessageType::Info);
        StatusLine::new_silent()
    }
}

fn clean_output_directory(path: &Path) -> anyhow::Result<()> {
    let to_delete = fs::read_dir(path)?
        .flatten()
        .map(|de| de.path())
        .filter(|p| p.is_dir()) // Only directories.
        .filter(|p| {
            let dir_str = p.to_string_lossy();
            dir_str.ends_with("_BurstDebugInformation_DoNotShip")
                || dir_str.ends_with("_BackUpThisFolder_ButDontShipItWithYourGame")
        });

    for dir in to_delete {
        println!("Removing directory: {}", dir.normalized_display());
        fs::remove_dir_all(&dir)
            .with_context(|| format!("Could not remove directory: {}", dir.normalized_display()))?;
    }

    Ok(())
}
