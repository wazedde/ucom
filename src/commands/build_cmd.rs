mod build_log;
mod editor;
mod script;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli_build::{BuildArguments, BuildMode, BuildOptions, BuildScriptTarget};
use crate::commands::unity_command_builder::UnityCommandBuilder;
use crate::commands::{ProjectSetup, TimeDeltaExt, report_version_issues};
use crate::unity::{
    ProjectPath, build_command_line, find_unity_editor_process, wait_with_log_output,
    wait_with_stdout,
};
use crate::utils::native_progress::{NativeProgressBar, NativeProgressState};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::{MessageType, StatusLine};
use anyhow::{Context, anyhow, bail};
use chrono::Utc;
use path_absolutize::Absolutize;

use build_log::{collect_log_errors, print_build_report, remove_old_log_file};
use editor::build_via_editor;
use script::csharp_build_script_injection_hooks;

/// Runs the build command.
pub fn build_project(arguments: &BuildArguments) -> anyhow::Result<()> {
    let start_time = Utc::now();
    let setup = ProjectSetup::new(&arguments.project_dir)?;

    // If the project is open in the Unity editor, build there via IPC.
    if let Some(editor_pid) = find_unity_editor_process(&setup.project) {
        return build_via_editor(arguments, &setup, editor_pid, start_time);
    }

    // Otherwise, build in a new Unity process.
    let editor_path = setup.editor_executable()?;

    let output_path = arguments.output_path(&setup.project)?;
    let log_path = arguments.full_log_path(&setup.project)?;

    let build_command = arguments.create_cmd(&setup.project, &editor_path, &output_path, &log_path);

    if arguments.dry_run {
        println!("{}", build_command_line(&build_command));
        return Ok(());
    }
    let build_status = start_build_status("Building", arguments, &setup);

    let _progress_guard = NativeProgressBar::with_state(NativeProgressState::Indeterminate);

    let hooks = csharp_build_script_injection_hooks(&setup.project, arguments.inject);

    (hooks.inject_build_script)()?;

    remove_old_log_file(&log_path)?;

    let build_result = if arguments.show_log() {
        wait_with_log_output(build_command, &log_path)
    } else {
        wait_with_stdout(build_command)
    };

    (hooks.cleanup_build_script)()?;
    drop(build_status);

    let (build_status, log_tag) = if build_result.is_ok() {
        if arguments.clean {
            clean_output_directory(&output_path)?;
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
            arguments.target,
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

    print_build_report(&log_path, build_status);
    report_version_issues(setup.unity_version);
    build_result.map_err(|_| collect_log_errors(&log_path))
}

/// Announces the start of a build: a status line in quiet mode, otherwise a printed message.
/// The status line is cleared when the returned value is dropped.
fn start_build_status(label: &str, args: &BuildArguments, setup: &ProjectSetup) -> StatusLine {
    let build_text = format!(
        "Unity {} {} project in {}",
        setup.unity_version,
        args.target,
        setup.project.normalized_display()
    );

    if args.quiet {
        StatusLine::new(label, &build_text)
    } else {
        MessageType::print_line(label, &build_text, MessageType::Info);
        StatusLine::new_silent()
    }
}

impl BuildArguments {
    /// Returns true if the log should be shown.
    fn show_log(&self) -> bool {
        !self.quiet && (self.mode == BuildMode::Batch || self.mode == BuildMode::BatchNoGraphics)
    }

    /// Returns the full path to the log file.
    /// By default, the project's `Logs` directory is used as destination.
    fn full_log_path(&self, project: &ProjectPath) -> anyhow::Result<PathBuf> {
        let log_file = self.log_file.as_deref().map_or_else(
            || format!("Build-{}.log", self.target).into(),
            ToOwned::to_owned,
        );

        let file_name = log_file
            .file_name()
            .ok_or_else(|| anyhow!("Invalid log file name: {}", log_file.normalized_display()))?;

        let path = if log_file == file_name {
            // Log filename without the path was given,
            // use the project's `Logs` directory as destination.
            project.join("Logs").join(file_name)
        } else {
            log_file
        };

        let path = path.absolutize()?.to_path_buf();
        Ok(path)
    }

    /// Returns the output path for the build.
    fn output_path(&self, project: &ProjectPath) -> anyhow::Result<PathBuf> {
        let output_dir = match &self.build_path {
            Some(path) => path.absolutize()?.into(),
            None => {
                // If no build path is given, use <project>/Builds/<target>
                project
                    .join("Builds")
                    .join(self.output_type.as_ref())
                    .join(self.target.as_ref())
            }
        };

        if project.as_ref() == output_dir {
            bail!(
                "Output directory cannot be the same as the project directory: {}",
                project.normalized_display()
            );
        }
        Ok(output_dir)
    }

    /// Creates the build command.
    fn create_cmd(
        &self,
        project: &ProjectPath,
        editor_exe: &Path,
        output_dir: &Path,
        log_file: &Path,
    ) -> Command {
        // Build the command using the builder pattern.
        let mut builder = UnityCommandBuilder::new(editor_exe.to_path_buf())
            .with_project_path(project.to_path_buf())
            .with_build_target(self.target.as_ref())
            .with_log_file(log_file)
            .add_arg("-executeMethod")
            .add_arg(&self.build_function)
            .add_arg("--ucom-build-output")
            .add_arg(output_dir.to_string_lossy().to_string())
            .add_arg("--ucom-build-target")
            .add_arg(BuildScriptTarget::from(self.target).as_ref());

        let build_options = self.build_option_flags();
        if build_options != (BuildOptions::None as i32) {
            builder = builder
                .add_arg("--ucom-build-options")
                .add_arg(build_options.to_string());
        }

        if let Some(build_args) = &self.build_args {
            builder = builder.add_arg("--ucom-pre-build-args").add_arg(build_args);
        }

        // Add the build mode flags.
        match self.mode {
            BuildMode::BatchNoGraphics => {
                builder = builder.batch_mode(true).no_graphics(true).quit(true);
            }
            BuildMode::Batch => {
                builder = builder.batch_mode(true).quit(true);
            }
            BuildMode::EditorQuit => {
                builder = builder.quit(true);
            }
            BuildMode::Editor => (), // Do nothing.
        }

        // Add any additional arguments.
        if let Some(a) = self.args.as_ref() {
            builder = builder.add_args(a.iter().cloned());
        }

        builder.build()
    }

    fn build_option_flags(&self) -> i32 {
        let mut option_flags = 0;
        if self.run_player {
            option_flags |= BuildOptions::AutoRunPlayer as i32;
        }

        if self.development_build {
            option_flags |= BuildOptions::Development as i32;
        }

        if self.show_built_player {
            option_flags |= BuildOptions::ShowBuiltPlayer as i32;
        }

        if self.allow_debugging {
            option_flags |= BuildOptions::AllowDebugging as i32;
        }

        if self.connect_with_profiler {
            option_flags |= BuildOptions::ConnectWithProfiler as i32;
        }

        if self.deep_profiling {
            option_flags |= BuildOptions::EnableDeepProfilingSupport as i32;
        }

        if self.connect_to_host {
            option_flags |= BuildOptions::ConnectToHost as i32;
        }

        let option_list = self
            .build_options
            .iter()
            .fold(0, |options, &o| options | (o as i32));

        option_flags | option_list
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
