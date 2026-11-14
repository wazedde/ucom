use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::cli_add::UnityTemplateFile;
use crate::cli_build::{
    BuildArguments, BuildMode, BuildOptions, BuildScriptTarget, DEFAULT_BUILD_FUNCTION,
    InjectAction,
};
use crate::commands::unity_command_builder::UnityCommandBuilder;
use crate::commands::{
    PERSISTENT_BUILD_SCRIPT_ROOT, ProjectSetup, TimeDeltaExt, add_file_to_project,
    report_version_issues, warn_non_fatal,
};
use crate::unity::{
    Pid, ProjectPath, build_command_line, comparable_path, find_unity_editor_process,
    is_process_running, wait_with_log_output, wait_with_stdout,
};
use crate::utils::native_progress::{NativeProgressBar, NativeProgressState};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::{MessageType, StatusLine};
use crate::utils::write_atomic;
use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, Utc};
use itertools::Itertools;
use path_absolutize::Absolutize;
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/Ucom";

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

fn print_build_report(log_path: &Path, status: MessageType) {
    if let Ok(file) = File::open(log_path) {
        // Iterate over lines from the build report in the log file.
        log_lines(file)
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

/// Returns the lines of a log file.
///
/// Invalid UTF-8, which native plugins or the OS can write in a local code page, is replaced
/// instead of ending the iteration.
fn log_lines(file: File) -> impl Iterator<Item = String> {
    BufReader::new(file)
        .split(b'\n')
        .map_while(Result::ok)
        .map(|line| {
            let mut line = String::from_utf8(line)
                .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
            if line.ends_with('\r') {
                line.pop();
            }
            line
        })
}

/// Returns the unique error lines in the given log file.
fn find_log_errors(log_file: &Path) -> std::io::Result<Vec<String>> {
    let file = File::open(log_file)?;
    Ok(log_lines(file)
        .filter(|l| line_contains_error(l))
        .unique()
        .collect_vec())
}

/// Returns errors from the given log file as one collected Err.
fn collect_log_errors(log_file: &Path) -> anyhow::Error {
    match find_log_errors(log_file) {
        Ok(errors) => format_log_errors(&errors),
        Err(_) => anyhow!("Failed to open log file: {}", log_file.normalized_display()),
    }
}

/// Combines error lines from a log into one error.
fn format_log_errors(errors: &[String]) -> anyhow::Error {
    match errors {
        [] => anyhow!("No errors found in log"),
        [single_error] => anyhow!("{single_error}"),
        _ => {
            let error_list = errors
                .iter()
                .enumerate()
                .map(|(i, error)| format!("{c}: {e}", c = i + 1, e = error))
                .join("\n");

            anyhow!(error_list)
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

/// Builds the project in the Unity editor that already has it open, using file-based IPC.
///
/// Flags that only control how a new Unity process is launched (`--mode`, `--inject`) do not
/// apply here. Flags that would change the build but cannot be sent to the editor produce a
/// warning.
fn build_via_editor(
    args: &BuildArguments,
    setup: &ProjectSetup,
    editor_pid: Pid,
    start_time: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    ensure_editor_builder_installed(setup)?;
    warn_unsupported_editor_build_flags(args);

    let output_path = args.output_path(&setup.project)?;
    let log_path = args.full_log_path(&setup.project)?;

    // The editor writes the build log next to the result, and ucom moves it to `log_path`.
    let uuid = Uuid::new_v4();
    let editor_log = editor_result_dir(&setup.project).join(format!("build-{uuid}.log"));

    let command = EditorCommand {
        command: EditorCommandKind::Build,
        uuid,
        timestamp: Utc::now(),
        platform: BuildScriptTarget::from(args.target),
        output_path: output_path.clone(),
        log_path: editor_log.clone(),
        build_options: args.build_option_flags(),
        development_build: args.development_build,
        force_platform_switch: args.force_editor_build,
        force_play_mode_exit: args.force_editor_build,
    };

    if args.dry_run {
        println!(
            "The project is open in the Unity editor. This build command would be sent to it:"
        );
        println!("{}", serde_json::to_string_pretty(&command)?);
        return Ok(());
    }

    let build_status = start_build_status("Building via editor", args, setup);

    let result = send_editor_command(&setup.project, &command, editor_pid)?;
    drop(build_status);

    let log_file = install_editor_log(&result, &editor_log, &log_path).unwrap_or_else(|e| {
        warn_non_fatal!("The build log was not written: {e:#}");
        None
    });

    if result.status == EditorBuildStatus::Success && args.clean {
        clean_output_directory(&output_path)?;
    }

    let outcome = handle_editor_build_result(&result, setup, start_time, log_file.as_deref());
    report_version_issues(setup.unity_version);
    outcome
}

/// Returns an error if the build script, which contains the editor command watcher,
/// is not installed in the project.
fn ensure_editor_builder_installed(setup: &ProjectSetup) -> anyhow::Result<()> {
    let builder_file_name = UnityTemplateFile::Builder.as_asset().filename;
    let builder_script_path = setup
        .project
        .join(PERSISTENT_BUILD_SCRIPT_ROOT)
        .join(builder_file_name);

    if !builder_script_path.exists() {
        bail!(
            "Unity editor is running, but {} not installed.\n\n\
             To enable building via the running editor, install the builder script:\n\
             \n\
             {}\n\
             \n\
             This will enable both batch mode and editor IPC builds.\n\
             Or close the Unity editor to build in batch mode.",
            builder_file_name,
            format_args!("  ucom add builder {}", setup.project.normalized_display())
        );
    }
    Ok(())
}

/// Warns about flags that change the build but are not supported when building via the editor.
fn warn_unsupported_editor_build_flags(args: &BuildArguments) {
    let mut unsupported = Vec::new();
    if args.build_function != DEFAULT_BUILD_FUNCTION {
        unsupported.push("--build-function");
    }
    if args.build_args.is_some() {
        unsupported.push("--build-args");
    }
    if args.args.as_ref().is_some_and(|a| !a.is_empty()) {
        unsupported.push("additional Unity arguments");
    }

    if !unsupported.is_empty() {
        warn_non_fatal!(
            "Ignored when building via the running Unity editor: {}",
            unsupported.join(", ")
        );
    }
}

/// Writes a command file for the editor and waits for the matching result file.
fn send_editor_command(
    project: &ProjectPath,
    command: &EditorCommand,
    editor_pid: Pid,
) -> anyhow::Result<EditorBuildResult> {
    let command_dir = project.join("Temp").join("ucom-commands");
    let result_dir = editor_result_dir(project);

    fs::create_dir_all(&command_dir)?;
    fs::create_dir_all(&result_dir)?;

    let file_name = format!("build-{}.json", command.uuid);
    let command_file = command_dir.join(&file_name);
    let result_file = result_dir.join(&file_name);

    write_atomic(&command_file, serde_json::to_string_pretty(command)?)?;

    let result = poll_for_result(&result_file, editor_pid)?;
    // NOTE: When the user cancels the polling, files are left behind. Which is fine.

    let _ = fs::remove_file(&command_file);
    let _ = fs::remove_file(&result_file);

    Ok(result)
}

/// Returns the directory where the Unity editor writes build results and logs.
fn editor_result_dir(project: &ProjectPath) -> PathBuf {
    project.join("Temp").join("ucom-results")
}

/// Moves the build log that the editor wrote to `editor_log` into place at `log_path`.
///
/// Returns the path of the log, or `None` if the editor wrote none. As with batch builds, a log
/// from an earlier build at `log_path` is removed first, unless `log_path` is the log file the
/// editor itself writes to.
fn install_editor_log(
    result: &EditorBuildResult,
    editor_log: &Path,
    log_path: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let outcome = (|| -> anyhow::Result<Option<PathBuf>> {
        if result
            .editor_log_path
            .as_deref()
            .is_some_and(|p| comparable_path(Path::new(p)) == comparable_path(log_path))
        {
            bail!(
                "{} is the log file of the Unity editor itself",
                log_path.normalized_display()
            );
        }

        remove_old_log_file(log_path)?;
        if !result.log_written {
            return Ok(None);
        }

        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Renaming fails when the log path is on another volume.
        if fs::rename(editor_log, log_path).is_err() {
            fs::copy(editor_log, log_path)?;
        }
        Ok(Some(log_path.to_path_buf()))
    })();

    let _ = fs::remove_file(editor_log);
    outcome
}

/// Polls for a result file from the Unity editor.
///
/// Returns the parsed result when the editor completes the build. There is no timeout, so
/// long builds are supported; the user can press Ctrl+C to cancel. Every few seconds the
/// editor process found at the start (`editor_pid`) is checked, and polling stops with an
/// error once it has exited.
///
/// Current versions of `UnityBuilder.cs` write the result file atomically. Builder scripts
/// installed in projects before that change write it in place, so a read may observe a
/// partially written file. Read and parse failures are therefore retried for a short grace
/// period before being reported.
fn poll_for_result(result_file: &Path, editor_pid: Pid) -> anyhow::Result<EditorBuildResult> {
    const POLL_INTERVAL: Duration = Duration::from_millis(500);
    const MAX_READ_ATTEMPTS: u32 = 10;
    const LIVENESS_CHECK_INTERVAL: Duration = Duration::from_secs(5);

    let _progress_guard = NativeProgressBar::with_state(NativeProgressState::Indeterminate);
    let mut failed_reads = 0;
    let mut last_liveness_check = Instant::now();

    loop {
        if result_file.exists() {
            match read_editor_build_result(result_file) {
                Ok(result) => return Ok(result),
                Err(e) => {
                    failed_reads += 1;
                    if failed_reads >= MAX_READ_ATTEMPTS {
                        return Err(e).with_context(|| {
                            format!(
                                "Failed to read build result from the editor: {}",
                                result_file.normalized_display()
                            )
                        });
                    }
                }
            }
        }

        if last_liveness_check.elapsed() >= LIVENESS_CHECK_INTERVAL {
            last_liveness_check = Instant::now();
            // The editor may have written the result just before exiting, so check for it again.
            if !is_process_running(editor_pid) && !result_file.exists() {
                bail!("The Unity editor is no longer running; no build result was received.");
            }
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Reads and parses a result file written by the Unity editor.
fn read_editor_build_result(result_file: &Path) -> anyhow::Result<EditorBuildResult> {
    let json = fs::read_to_string(result_file)?;
    Ok(serde_json::from_str(&json)?)
}

/// Handles the result from an editor build, displaying appropriate messages.
///
/// Returns `Ok(())` if the build succeeded, `Err` otherwise.
fn handle_editor_build_result(
    result: &EditorBuildResult,
    setup: &ProjectSetup,
    start_time: chrono::DateTime<Utc>,
    log_file: Option<&Path>,
) -> anyhow::Result<()> {
    match result.status {
        EditorBuildStatus::Error => bail!("{}", result.message),
        EditorBuildStatus::Failed => {
            // Check for platform switch failure specifically
            if result.error_code == Some(EditorErrorCode::PlatformSwitchFailed) {
                return Err(editor_build_error(result, log_file).context(
                    "Platform switch failed. This may happen if:\n\
                     - The target platform is not installed\n\
                     - The user cancelled the operation\n\
                     - Unity encountered an error during the switch",
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

            // Only builds that ran have a report.
            if result.build_result.is_some() {
                print_editor_build_report(result, MessageType::Error, log_file);
            }
        }
        EditorBuildStatus::Success => {
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
            print_editor_build_report(result, MessageType::Ok, log_file);
        }
        EditorBuildStatus::Unknown => {}
    }

    let succeeded = result.status == EditorBuildStatus::Success;

    MessageType::print_line(
        "Total time",
        format!(
            "{t:.2}s",
            t = Utc::now().signed_duration_since(start_time).as_seconds()
        ),
        if succeeded {
            MessageType::Ok
        } else {
            MessageType::Error
        },
    );

    if succeeded {
        Ok(())
    } else {
        Err(editor_build_error(result, log_file))
    }
}

/// Returns the error for a failed editor build: the errors from its log if there are any,
/// otherwise the editor's message.
fn editor_build_error(result: &EditorBuildResult, log_file: Option<&Path>) -> anyhow::Error {
    // Older builder scripts do not write a log file.
    let Some(log_path) = log_file else {
        return anyhow!("{}", result.message);
    };

    match find_log_errors(log_path) {
        Ok(errors) if !errors.is_empty() => format_log_errors(&errors),
        _ => anyhow!(
            "{}\nSee the build log: {}",
            result.message,
            log_path.normalized_display()
        ),
    }
}

/// Removes the log file of a previous build, so a stale log is never reported.
fn remove_old_log_file(log_path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(log_path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).with_context(|| {
            format!(
                "Failed to remove old log file: {}",
                log_path.normalized_display()
            )
        }),
        _ => Ok(()),
    }
}

/// Prints the build report from editor build result.
fn print_editor_build_report(
    result: &EditorBuildResult,
    status: MessageType,
    log_file: Option<&Path>,
) {
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

    if let Some(log_file) = log_file {
        MessageType::print_line(
            "Log file",
            log_file.normalized_display().to_string(),
            status,
        );
    }
}

/// Command structure sent to Unity editor via JSON file.
///
/// Read by `CommandFile` in `UnityBuilder.cs`, which treats every field as a plain string,
/// number or boolean.
#[derive(Serialize, Debug)]
struct EditorCommand {
    command: EditorCommandKind,
    uuid: Uuid,
    timestamp: DateTime<Utc>,
    platform: BuildScriptTarget,
    output_path: PathBuf,
    log_path: PathBuf,
    build_options: i32,
    development_build: bool,
    force_platform_switch: bool,
    force_play_mode_exit: bool,
}

/// The commands the editor command watcher understands.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum EditorCommandKind {
    Build,
}

/// Result structure received from Unity editor via JSON file (`ResultFile` in `UnityBuilder.cs`).
///
/// Unity's `JsonUtility` writes every field, using `""` for unset strings and `0` for unset
/// numbers. Unknown fields are ignored, so newer builder scripts stay readable.
#[derive(Deserialize, Debug)]
struct EditorBuildResult {
    status: EditorBuildStatus,
    message: String,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    error_code: Option<EditorErrorCode>,
    platform_switched: bool,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    original_platform: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    switched_to: Option<String>,
    build_time_seconds: f32,
    platform_switch_time_seconds: f32,
    output_path: String,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    build_result: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    platform: Option<String>,
    total_size: Option<u64>,
    total_errors: Option<i32>,
    total_warnings: Option<i32>,
    /// Whether the editor wrote the build log to the command's `log_path`. Older builder scripts
    /// do not write a log, and leave this out.
    #[serde(default)]
    log_written: bool,
    /// The log file the editor itself writes to, if any.
    #[serde(default, deserialize_with = "empty_string_as_none")]
    editor_log_path: Option<String>,
}

/// The outcome of an editor command.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum EditorBuildStatus {
    /// The build succeeded.
    Success,
    /// The build ran, or started to run, and failed.
    Failed,
    /// The command was rejected before building, for example because the editor is in Play Mode.
    Error,
    /// A status from a newer builder script.
    #[serde(other)]
    Unknown,
}

/// Error codes written by `UnityBuilder.cs`.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum EditorErrorCode {
    PlatformMismatch,
    InPlayMode,
    Compiling,
    InvalidPlatform,
    PlatformSwitchFailed,
    BuildFailed,
    UnknownCommand,
    InternalError,
    /// A code from a newer builder script.
    #[serde(other)]
    Unknown,
}

/// Deserializes an optional value from a string, treating `""` as `None`.
///
/// Unity's `JsonUtility` writes unset string fields as empty strings instead of `null`.
fn empty_string_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        Some(s) if !s.is_empty() => {
            T::deserialize(IntoDeserializer::<D::Error>::into_deserializer(s)).map(Some)
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_util::TempDir;

    /// Returns the fields every result from the editor has.
    fn base_result_json() -> serde_json::Value {
        serde_json::json!({
            "uuid": "1234",
            "status": "success",
            "message": "ok",
            "platform_switched": false,
            "build_time_seconds": 1.0,
            "platform_switch_time_seconds": 0.0,
            "output_path": "/tmp/out",
        })
    }

    /// Returns a failed build result, as `UnityBuilder.cs` writes it.
    fn failed_result() -> EditorBuildResult {
        let mut json = base_result_json();
        json["status"] = "failed".into();
        json["message"] = "Build failed. Check Unity console for errors.".into();
        json["error_code"] = "BUILD_FAILED".into();
        serde_json::from_value(json).unwrap()
    }

    /// Returns a result in which the editor reports its own log file and whether it wrote the build log.
    fn result_with_log(editor_log_path: &Path, log_written: bool) -> EditorBuildResult {
        let mut json = base_result_json();
        json["editor_log_path"] = editor_log_path.to_string_lossy().into_owned().into();
        json["log_written"] = log_written.into();
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn editor_build_result_treats_empty_strings_as_none() {
        // Shaped like the output of Unity's `JsonUtility` for a build without a platform switch.
        let json = r#"{
            "uuid": "1234",
            "status": "success",
            "message": "ok",
            "error_code": "",
            "platform_switched": false,
            "original_platform": "StandaloneOSX",
            "switched_to": "",
            "build_time_seconds": 1.5,
            "platform_switch_time_seconds": 0.0,
            "output_path": "/tmp/out",
            "build_result": "Succeeded",
            "platform": "StandaloneOSX",
            "total_size": 0,
            "total_errors": 0,
            "total_warnings": 0,
            "log_written": true,
            "editor_log_path": ""
        }"#;

        let result: EditorBuildResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.editor_log_path, None);
        assert_eq!(result.error_code, None);
        assert_eq!(result.switched_to, None);
        assert_eq!(result.original_platform.as_deref(), Some("StandaloneOSX"));
        assert_eq!(result.build_result.as_deref(), Some("Succeeded"));
        assert!(result.log_written);
    }

    #[test]
    fn editor_build_result_reads_rejected_command() {
        // Written by `WriteErrorResult` in `UnityBuilder.cs`: only a few fields are set.
        let json = r#"{
            "uuid": "1234",
            "status": "error",
            "message": "Unity editor is in Play Mode. Use --force-editor-build to exit play mode.",
            "error_code": "IN_PLAY_MODE",
            "platform_switched": false,
            "original_platform": "",
            "switched_to": "",
            "build_time_seconds": 0.0,
            "platform_switch_time_seconds": 0.0,
            "output_path": "",
            "build_result": "",
            "platform": "",
            "total_size": 0,
            "total_errors": 0,
            "total_warnings": 0,
            "log_written": false,
            "editor_log_path": ""
        }"#;

        let result: EditorBuildResult = serde_json::from_str(json).unwrap();

        assert_eq!(result.status, EditorBuildStatus::Error);
        assert_eq!(result.error_code, Some(EditorErrorCode::InPlayMode));
        assert_eq!(result.build_result, None);
    }

    #[test]
    fn editor_build_result_maps_unrecognised_status_and_code_to_unknown() {
        let mut json = base_result_json();
        json["status"] = "cancelled".into();
        json["error_code"] = "SOMETHING_NEW".into();

        let result: EditorBuildResult = serde_json::from_value(json).unwrap();

        assert_eq!(result.status, EditorBuildStatus::Unknown);
        assert_eq!(result.error_code, Some(EditorErrorCode::Unknown));
    }

    #[test]
    fn editor_command_serializes_as_the_builder_script_expects() {
        let uuid = Uuid::parse_str("0b5c3a3e-5f37-4c2b-9d0a-2f1f4c8e9a10").unwrap();
        let command = EditorCommand {
            command: EditorCommandKind::Build,
            uuid,
            timestamp: DateTime::parse_from_rfc3339("2026-11-14T10:37:05Z")
                .unwrap()
                .with_timezone(&Utc),
            platform: BuildScriptTarget::StandaloneOSX,
            output_path: PathBuf::from("/project/Builds/Release/OSXUniversal"),
            log_path: PathBuf::from("/project/Temp/ucom-results/build-1.log"),
            build_options: 1,
            development_build: true,
            force_platform_switch: false,
            force_play_mode_exit: false,
        };

        let json = serde_json::to_value(&command).unwrap();

        // `CommandFile` in `UnityBuilder.cs` reads these as plain strings.
        assert_eq!(json["command"], "build");
        assert_eq!(json["uuid"], "0b5c3a3e-5f37-4c2b-9d0a-2f1f4c8e9a10");
        assert_eq!(json["timestamp"], "2026-11-14T10:37:05Z");
        assert_eq!(json["platform"], "StandaloneOSX");
        assert_eq!(json["output_path"], "/project/Builds/Release/OSXUniversal");
        assert_eq!(json["log_path"], "/project/Temp/ucom-results/build-1.log");
        assert_eq!(json["build_options"], 1);
        assert_eq!(json["development_build"], true);
    }

    #[test]
    fn editor_build_result_accepts_missing_and_null_optional_fields() {
        let mut json = base_result_json();
        json["switched_to"] = serde_json::Value::Null;

        let result: EditorBuildResult = serde_json::from_value(json).unwrap();

        assert_eq!(result.original_platform, None);
        assert_eq!(result.switched_to, None);
        assert_eq!(result.platform, None);
        assert!(!result.log_written);
    }

    #[test]
    fn remove_old_log_file_removes_existing_and_ignores_missing() {
        let dir = TempDir::new("remove-old-log");
        let log_path = dir.path().join("Build-macos.log");

        remove_old_log_file(&log_path).unwrap();

        fs::write(&log_path, "old log").unwrap();
        remove_old_log_file(&log_path).unwrap();
        assert!(!log_path.exists());
    }

    #[test]
    fn install_editor_log_moves_the_log_into_place() {
        let dir = TempDir::new("install-log-move");
        let editor_log = dir.path().join("build-1234.log");
        let log_path = dir.path().join("Logs").join("Build.log");
        fs::write(&editor_log, "new log").unwrap();

        let result = result_with_log(&dir.path().join("Editor.log"), true);
        let installed = install_editor_log(&result, &editor_log, &log_path).unwrap();

        assert_eq!(installed.as_deref(), Some(log_path.as_path()));
        assert_eq!(fs::read_to_string(&log_path).unwrap(), "new log");
        assert!(!editor_log.exists());
    }

    #[test]
    fn install_editor_log_removes_stale_log_when_none_was_written() {
        let dir = TempDir::new("install-log-stale");
        let log_path = dir.path().join("Build.log");
        fs::write(&log_path, "old log").unwrap();

        let result = result_with_log(&dir.path().join("Editor.log"), false);
        let installed =
            install_editor_log(&result, &dir.path().join("build-1234.log"), &log_path).unwrap();

        assert_eq!(installed, None);
        assert!(!log_path.exists());
    }

    #[test]
    fn install_editor_log_leaves_the_editors_own_log_alone() {
        let dir = TempDir::new("install-log-editor");
        let editor_log = dir.path().join("build-1234.log");
        let editor_own_log = dir.path().join("Editor.log");
        fs::write(&editor_log, "new log").unwrap();
        fs::write(&editor_own_log, "editor log").unwrap();

        let result = result_with_log(&editor_own_log, true);

        assert!(install_editor_log(&result, &editor_log, &editor_own_log).is_err());
        assert_eq!(fs::read_to_string(&editor_own_log).unwrap(), "editor log");
        assert!(!editor_log.exists());
    }

    #[test]
    fn editor_build_error_uses_errors_from_log() {
        let dir = TempDir::new("editor-error-log");
        let log_path = dir.path().join("Build.log");
        fs::write(
            &log_path,
            "Compiling\nAssets/A.cs(5,31): error CS0029: Cannot convert\nError building Player because scripts had compiler errors\n",
        )
        .unwrap();

        let error = editor_build_error(&failed_result(), Some(&log_path)).to_string();

        assert!(error.contains("1: Assets/A.cs(5,31): error CS0029: Cannot convert"));
        assert!(error.contains("2: Error building Player"));
    }

    #[test]
    fn editor_build_error_falls_back_to_message_when_log_has_no_errors() {
        let dir = TempDir::new("editor-error-no-errors");
        let log_path = dir.path().join("Build.log");
        fs::write(&log_path, "Build Finished, Result: Failure.\n").unwrap();

        let error = editor_build_error(&failed_result(), Some(&log_path)).to_string();

        assert!(error.starts_with("Build failed. Check Unity console for errors."));
        assert!(error.contains("Build.log"));
    }

    #[test]
    fn editor_build_error_uses_message_without_log() {
        let error = editor_build_error(&failed_result(), None).to_string();
        assert_eq!(error, "Build failed. Check Unity console for errors.");
    }

    #[test]
    fn find_log_errors_reads_past_invalid_utf8() {
        let dir = TempDir::new("log-invalid-utf8");
        let log_path = dir.path().join("Build.log");
        fs::write(
            &log_path,
            b"Plugin says \xE9t\xE9\r\nAssets/A.cs(1,1): error CS1002: ; expected\r\n",
        )
        .unwrap();

        let errors = find_log_errors(&log_path).unwrap();

        assert_eq!(errors, ["Assets/A.cs(1,1): error CS1002: ; expected"]);
    }
}
