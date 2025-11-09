use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::cli_add::UnityTemplateFile;
use crate::cli_build::{BuildArguments, BuildMode, BuildOptions, BuildScriptTarget, InjectAction};
use crate::commands::{
    PERSISTENT_BUILD_SCRIPT_ROOT, ProjectSetup, TimeDeltaExt, UnityCommandBuilder,
    add_file_to_project, check_version_issues,
};
use crate::unity::{
    ProjectPath, build_command_line, is_unity_editor_running, wait_with_log_output,
    wait_with_stdout,
};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::{MessageType, StatusLine};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use itertools::Itertools;
use path_absolutize::Absolutize;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/Ucom";

/// Runs the build command.
pub fn build_project(arguments: &BuildArguments) -> anyhow::Result<()> {
    let start_time = Utc::now();
    let setup = ProjectSetup::new(&arguments.project_dir)?;

    // Try to build via editor IPC if editor is running
    if let Some(result) = try_editor_build(arguments, &setup)? {
        return handle_editor_build_result(result, &setup, start_time);
    }

    // Fall back to batch mode build
    let editor_path = setup.editor_executable()?;

    let output_path = arguments.output_path(&setup.project)?;
    let log_path = arguments.full_log_path(&setup.project)?;

    let build_command = arguments.create_cmd(&setup.project, &editor_path, &output_path, &log_path);

    if arguments.dry_run {
        println!("{}", build_command_line(&build_command));
        return Ok(());
    }
    let build_text = format!(
        "Unity {} {} project in {}",
        setup.unity_version,
        arguments.target,
        setup.project.normalized_display()
    );

    let build_status = if arguments.quiet {
        StatusLine::new("Building", &build_text)
    } else {
        MessageType::print_line("Building", &build_text, MessageType::Info);
        StatusLine::new_silent()
    };

    let hooks = csharp_build_script_injection_hooks(&setup.project, arguments.inject);

    (hooks.inject_build_script)()?;

    if log_path.exists() {
        fs::remove_file(&log_path)?;
    }

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
    check_version_issues(setup.unity_version);
    build_result.map_err(|_| collect_log_errors(&log_path))
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
            std::borrow::ToOwned::to_owned,
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
            return Err(anyhow!(
                "Output directory cannot be the same as the project directory: {}",
                project.normalized_display()
            ));
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

fn print_build_report(log_path: &Path, status: MessageType) {
    if let Ok(file) = File::open(log_path) {
        // Iterate over lines from the build report in the log file.
        BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .skip_while(|l| !l.starts_with("[Builder] Build Report")) // Find marker.
            .skip(1) // Skip the marker.
            .take_while(|l| !l.is_empty()) // Read until empty line.
            .for_each(|l| {
                if let Some((key, value)) = l.split_once(':') {
                    MessageType::print_line(key.trim(), value.trim(), status);
                } else {
                    println!("{l}");
                }
            });
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

/// Returns errors from the given log file as one collected Err.
fn collect_log_errors(log_file: &Path) -> anyhow::Error {
    let Ok(log_file) = File::open(log_file) else {
        return anyhow!("Failed to open log file: {}", log_file.normalized_display());
    };

    let errors = BufReader::new(log_file)
        .lines()
        .map_while(Result::ok)
        .filter(|l| line_contains_error(l))
        .unique()
        .collect_vec();

    match &errors[..] {
        [] => anyhow!("No errors found in log"),
        [single_error] => anyhow!("{single_error}"),
        _ => {
            let joined = errors
                .iter()
                .enumerate()
                .map(|(i, error)| format!("{c}: {e}", c = i + 1, e = error))
                .join("\n");

            anyhow!(joined)
        }
    }
}

/// Returns true if the given line is an error.
fn line_contains_error(line: &str) -> bool {
    const ERROR_PREFIXES: &[&str] = &[
        "[Builder] Error:",
        "error CS",
        "Fatal Error",
        "Error building Player",
        "error:",
        "BuildFailedException:",
        "System.Exception:",
    ];

    ERROR_PREFIXES.iter().any(|prefix| line.contains(prefix))
}

/// Represents a hook function that returns a result.
type HookFn = Box<dyn FnOnce() -> anyhow::Result<()>>;

/// Returns a no-op hook function.
fn no_op_hook() -> HookFn {
    Box::new(|| Ok(()))
}

/// Represents the build hooks for injecting and cleaning up the build script.
struct BuildHooks {
    inject_build_script: HookFn,
    cleanup_build_script: HookFn,
}

impl BuildHooks {
    /// Creates a new `BuildHooks` instance with the specified hooks.
    fn new(inject_build_script: HookFn, cleanup_build_script: HookFn) -> Self {
        Self {
            inject_build_script,
            cleanup_build_script,
        }
    }

    /// Returns a no-op `BuildHooks` instance.
    fn no_op() -> Self {
        Self::new(no_op_hook(), no_op_hook())
    }
}

/// Creates actions that inject a script into the project before and after the build.
fn csharp_build_script_injection_hooks(project: &ProjectPath, inject: InjectAction) -> BuildHooks {
    let persistent_script_exists = project
        .join(PERSISTENT_BUILD_SCRIPT_ROOT)
        .join(UnityTemplateFile::Builder.as_asset().filename)
        .exists();

    match inject {
        // Build script is already present, no need to inject.
        InjectAction::Auto | InjectAction::Persistent if persistent_script_exists => {
            BuildHooks::no_op()
        }

        // Build script is not present, inject it in a unique directory to avoid conflicts.
        InjectAction::Auto => {
            let uuid = Uuid::new_v4();
            let unique_dir_name = format!("{AUTO_BUILD_SCRIPT_ROOT}-{uuid}");
            let closure_project_dir = project.to_path_buf();
            let closure_script_dir = PathBuf::from(&unique_dir_name).join("Editor");
            let closure_remove_dir = project.join(&unique_dir_name);

            BuildHooks::new(
                Box::new(|| {
                    add_file_to_project(
                        closure_project_dir,
                        closure_script_dir,
                        UnityTemplateFile::Builder,
                    )
                }),
                Box::new(|| cleanup_csharp_build_script(closure_remove_dir)),
            )
        }

        // Build script is not present, inject it.
        InjectAction::Persistent => {
            let closure_project_dir = project.to_path_buf();
            let closure_script_dir = PathBuf::from(PERSISTENT_BUILD_SCRIPT_ROOT);

            BuildHooks::new(
                Box::new(|| {
                    add_file_to_project(
                        closure_project_dir,
                        closure_script_dir,
                        UnityTemplateFile::Builder,
                    )
                }),
                no_op_hook(),
            )
        }

        // No need to do anything.
        InjectAction::Off => BuildHooks::no_op(),
    }
}

/// Removes the injected build script from the project.
fn cleanup_csharp_build_script(parent_dir: impl AsRef<Path>) -> anyhow::Result<()> {
    let parent_dir = parent_dir.as_ref();
    if !parent_dir.exists() {
        return Ok(());
    }

    // Remove the directory where the build script is located.
    fs::remove_dir_all(parent_dir).with_context(|| {
        format!(
            "Could not remove temporary directory: {}",
            parent_dir.normalized_display()
        )
    })?;

    // Remove the .meta file.
    let meta_file = parent_dir.with_extension("meta");
    if meta_file.exists() {
        fs::remove_file(&meta_file)
            .with_context(|| format!("Could not remove: {}", meta_file.normalized_display()))?;
    }

    Ok(())
}

/// Attempts to build the project using an already-running Unity editor via IPC.
///
/// Returns `Ok(Some(result))` if the editor is running and responds.
/// Returns `Ok(None)` if the editor is not running (caller should fall back to batch mode).
/// Returns `Err` if communication fails or the build fails.
fn try_editor_build(
    args: &BuildArguments,
    setup: &ProjectSetup,
) -> Result<Option<EditorBuildResult>> {
    if !is_unity_editor_running(&setup.project)? {
        return Ok(None);
    }

    // Check if UnityBuilder.cs is installed (contains the editor watcher)
    let builder_file_name = UnityTemplateFile::Builder.as_asset().filename;
    let builder_script_path = setup
        .project
        .join(PERSISTENT_BUILD_SCRIPT_ROOT)
        .join(builder_file_name);

    if !builder_script_path.exists() {
        return Err(anyhow!(
            "Unity editor is running, but {} not installed.\n\n\
             To enable building via the running editor, install the builder script:\n\
             \n\
             {}\n\
             \n\
             This will enable both batch mode and editor IPC builds.\n\
             Or close the Unity editor to build in batch mode.",
            builder_file_name,
            format_args!("  ucom add builder {}", setup.project.normalized_display())
        ));
    }

    MessageType::print_line(
        "Building via editor",
        format!(
            "Unity {} {} project in {}",
            setup.unity_version,
            args.target,
            setup.project.normalized_display()
        ),
        MessageType::Info,
    );

    let uuid = Uuid::new_v4();
    let temp_dir = setup.project.join("Temp");
    let command_dir = temp_dir.join("ucom-commands");
    let result_dir = temp_dir.join("ucom-results");

    fs::create_dir_all(&command_dir)?;
    fs::create_dir_all(&result_dir)?;

    let output_path = args.output_path(&setup.project)?;
    let log_path = args.full_log_path(&setup.project)?;

    let command = EditorCommand {
        command: "build".to_string(),
        uuid: uuid.to_string(),
        timestamp: Utc::now().to_rfc3339(),
        platform: BuildScriptTarget::from(args.target).as_ref().to_string(),
        output_path: output_path.to_string_lossy().to_string(),
        log_path: log_path.to_string_lossy().to_string(),
        build_options: args.build_option_flags(),
        development_build: args.development_build,
        force_platform_switch: args.force_editor_build,
        force_play_mode_exit: args.force_editor_build,
    };

    let command_file = command_dir.join(format!("build-{uuid}.json"));
    let command_json = serde_json::to_string_pretty(&command)?;
    fs::write(&command_file, command_json)?;

    let result = poll_for_result(&result_dir, &uuid)?;
    // NOTE: When the user cancels the polling, files are left behind. Which is fine.

    let _ = fs::remove_file(&command_file);
    let result_file = result_dir.join(format!("build-{uuid}.json"));
    let _ = fs::remove_file(&result_file);

    Ok(Some(result))
}

/// Polls for a result file from the Unity editor.
///
/// Returns the parsed result when the editor completes the build.
/// Polls indefinitely - user can press Ctrl+C to cancel.
fn poll_for_result(result_dir: &Path, uuid: &Uuid) -> Result<EditorBuildResult> {
    let result_file = result_dir.join(format!("build-{uuid}.json"));
    let poll_interval = Duration::from_millis(500);

    loop {
        if result_file.exists() {
            let json = fs::read_to_string(&result_file)?;
            let result: EditorBuildResult = serde_json::from_str(&json)?;
            return Ok(result);
        }

        std::thread::sleep(poll_interval);
    }
}

/// Handles the result from an editor build, displaying appropriate messages.
///
/// Returns `Ok(())` if the build succeeded, `Err` otherwise.
fn handle_editor_build_result(
    result: EditorBuildResult,
    setup: &ProjectSetup,
    start_time: chrono::DateTime<Utc>,
) -> Result<()> {
    match result.status.as_str() {
        "error" => return Err(anyhow!("{}", result.message)),
        "failed" => {
            // Check for platform switch failure specifically
            if result.error_code.as_deref() == Some("PLATFORM_SWITCH_FAILED") {
                return Err(anyhow!(
                    "Platform switch failed: {}\n\n\
                     This may happen if:\n\
                     - The target platform is not installed\n\
                     - The user cancelled the operation\n\
                     - Unity encountered an error during the switch",
                    result.message
                ));
            }

            MessageType::print_line(
                "Failed",
                format!(
                    "building Unity {} {} project",
                    setup.unity_version,
                    result.original_platform.as_deref().unwrap_or("Unknown")
                ),
                MessageType::Error,
            );
        }
        "success" => {
            if result.platform_switched {
                MessageType::print_line(
                    "Platform switched",
                    format!(
                        "{} → {} ({:.1}s)",
                        result.original_platform.as_deref().unwrap_or("Unknown"),
                        result.switched_to.as_deref().unwrap_or("Unknown"),
                        result.platform_switch_time_seconds
                    ),
                    MessageType::Info,
                );
            }

            MessageType::print_line(
                "Succeeded",
                format!(
                    "building Unity {} {} project in {}",
                    setup.unity_version,
                    result
                        .switched_to
                        .as_deref()
                        .or(result.original_platform.as_deref())
                        .unwrap_or("Unknown"),
                    setup.project.normalized_display()
                ),
                MessageType::Ok,
            );

            // Print build report
            print_editor_build_report(&result, MessageType::Ok);
        }
        _ => {}
    }

    MessageType::print_line(
        "Total time",
        format!(
            "{t:.2}s",
            t = Utc::now().signed_duration_since(start_time).as_seconds()
        ),
        if result.status == "success" {
            MessageType::Ok
        } else {
            MessageType::Error
        },
    );

    if result.status == "success" {
        Ok(())
    } else {
        Err(anyhow!("Build failed"))
    }
}

/// Prints the build report from editor build result.
fn print_editor_build_report(result: &EditorBuildResult, status: MessageType) {
    if let Some(build_result) = &result.build_result {
        MessageType::print_line("Build result", build_result, status);
    }

    if let Some(platform) = &result.platform {
        MessageType::print_line("Platform", platform, status);
    }

    MessageType::print_line("Output path", &result.output_path, status);

    if let Some(total_size) = result.total_size {
        MessageType::print_line("Size", format!("{} MB", total_size / 1024 / 1024), status);
    }

    if let Some(total_errors) = result.total_errors {
        MessageType::print_line("Errors", total_errors.to_string(), status);
    }

    if let Some(total_warnings) = result.total_warnings {
        MessageType::print_line("Warnings", total_warnings.to_string(), status);
    }

    MessageType::print_line(
        "Build time",
        format!("{:.2}s", result.build_time_seconds),
        status,
    );
}

/// Command structure sent to Unity editor via JSON file.
#[derive(Serialize, Debug)]
struct EditorCommand {
    command: String,
    uuid: String,
    timestamp: String,
    platform: String,
    output_path: String,
    log_path: String,
    build_options: i32,
    development_build: bool,
    force_platform_switch: bool,
    force_play_mode_exit: bool,
}

/// Result structure received from Unity editor via JSON file.
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct EditorBuildResult {
    uuid: String,
    status: String,
    message: String,
    error_code: Option<String>,
    platform_switched: bool,
    original_platform: Option<String>,
    switched_to: Option<String>,
    build_time_seconds: f32,
    platform_switch_time_seconds: f32,
    output_path: String,
    build_result: Option<String>,
    platform: Option<String>,
    total_size: Option<u64>,
    total_errors: Option<i32>,
    total_warnings: Option<i32>,
}
