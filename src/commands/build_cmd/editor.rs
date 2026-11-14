//! Building in a Unity editor that already has the project open, through command and result
//! files (editor IPC). The editor side is `EditorCommandWatcher` in `UnityBuilder.cs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use chrono::{DateTime, Utc};
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::build_log::{find_log_errors, format_log_errors, remove_old_log_file};
use super::plan::BuildPlan;
use super::script::persistent_builder_path;
use super::{clean_output_directory, start_build_status};
use crate::cli_add::UnityTemplateFile;
use crate::cli_build::{BuildOptions, BuildScriptTarget};
use crate::commands::{ProjectSetup, TimeDeltaExt, report_version_issues, warn_non_fatal};
use crate::unity::{Pid, ProjectPath, comparable_path, is_process_running};
use crate::utils::native_progress::{NativeProgressBar, NativeProgressState};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::MessageType;
use crate::utils::write_atomic;

/// Builds the project in the Unity editor that already has it open, using file-based IPC.
///
/// Flags that only control how a new Unity process is launched (`--mode`, `--inject`) do not
/// apply here. Flags that would change the build but cannot be sent to the editor produce a
/// warning.
pub(super) fn build_via_editor(
    plan: &BuildPlan,
    setup: &ProjectSetup,
    editor_pid: Pid,
    start_time: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    if let Err(e) = ensure_editor_builder_installed(setup) {
        if !plan.args.dry_run {
            return Err(e);
        }
        // A dry run still shows the command, and reports the missing script as a warning.
        warn_non_fatal!("{e:#}");
    }
    warn_unsupported_editor_build_flags(plan);

    // The editor writes the build log next to the result, and ucom moves it to `log_path`.
    let uuid = Uuid::new_v4();
    let editor_log = editor_result_dir(&setup.project).join(format!("build-{uuid}.log"));

    let command = EditorCommand {
        command: EditorCommandKind::Build,
        uuid,
        timestamp: Utc::now(),
        platform: plan.script_target(),
        output_path: plan.output_path.clone(),
        log_path: editor_log.clone(),
        build_options: plan.options.bits(),
        development_build: plan.options.contains(BuildOptions::Development),
        force_platform_switch: plan.args.force_editor_build,
        force_play_mode_exit: plan.args.force_editor_build,
    };
    let command_json = serde_json::to_string_pretty(&command).context(
        "Failed to create the build command for the Unity editor; are all paths valid UTF-8?",
    )?;

    if plan.args.dry_run {
        println!(
            "The project is open in the Unity editor. This build command would be sent to it:"
        );
        println!("{command_json}");
        println!(
            "The editor writes the build log to the log_path above; ucom then moves it to: {}",
            plan.log_path.normalized_display()
        );
        return Ok(());
    }

    let build_status = start_build_status("Building via editor", plan, setup);

    let result = send_editor_command(&setup.project, uuid, &command_json, editor_pid)?;
    drop(build_status);

    let log_file = install_editor_log(&result, &editor_log, &plan.log_path).unwrap_or_else(|e| {
        warn_non_fatal!("The build log was not written: {e:#}");
        None
    });

    if result.status == EditorBuildStatus::Success && plan.args.clean {
        clean_output_directory(&plan.output_path)?;
    }

    let outcome = handle_editor_build_result(&result, setup, start_time, log_file.as_deref());
    report_version_issues(setup.unity_version);
    outcome
}

/// Returns an error if the build script, which contains the editor command watcher,
/// is not installed in the project.
fn ensure_editor_builder_installed(setup: &ProjectSetup) -> anyhow::Result<()> {
    let builder_file_name = UnityTemplateFile::Builder.as_asset().filename;

    if !persistent_builder_path(&setup.project).exists() {
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
fn warn_unsupported_editor_build_flags(plan: &BuildPlan) {
    let unsupported = plan.unsupported_in_editor();
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
    uuid: Uuid,
    command_json: &str,
    editor_pid: Pid,
) -> anyhow::Result<EditorBuildResult> {
    let command_dir = project.join("Temp").join("ucom-commands");
    let result_dir = editor_result_dir(project);

    fs::create_dir_all(&command_dir)?;
    fs::create_dir_all(&result_dir)?;

    let file_name = format!("build-{uuid}.json");
    let command_file = command_dir.join(&file_name);
    let result_file = result_dir.join(&file_name);

    write_atomic(&command_file, command_json)?;

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
fn poll_for_result(result_file: &Path, editor_pid: Pid) -> anyhow::Result<EditorBuildResult> {
    let _progress_guard = NativeProgressBar::with_state(NativeProgressState::Indeterminate);
    wait_for_result(result_file, PollIntervals::DEFAULT, || {
        is_process_running(editor_pid)
    })
}

/// How often to look for the result file, and to check that the editor is still running.
#[derive(Debug, Clone, Copy)]
struct PollIntervals {
    result: Duration,
    liveness: Duration,
}

impl PollIntervals {
    const DEFAULT: Self = Self {
        result: Duration::from_millis(500),
        liveness: Duration::from_secs(5),
    };
}

/// Waits until `result_file` holds a result, or `editor_running` reports that the editor exited
/// without writing one.
///
/// Current versions of `UnityBuilder.cs` write the result file atomically. Builder scripts
/// installed in projects before that change write it in place, so a read may observe a
/// partially written file. Read and parse failures are therefore retried for a short grace
/// period before being reported.
fn wait_for_result(
    result_file: &Path,
    intervals: PollIntervals,
    mut editor_running: impl FnMut() -> bool,
) -> anyhow::Result<EditorBuildResult> {
    const MAX_READ_ATTEMPTS: u32 = 10;

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

        if last_liveness_check.elapsed() >= intervals.liveness {
            last_liveness_check = Instant::now();
            // The editor may have written the result just before exiting, so check for it again.
            if !editor_running() && !result_file.exists() {
                bail!("The Unity editor is no longer running; no build result was received.");
            }
        }

        std::thread::sleep(intervals.result);
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

    /// Intervals that make `wait_for_result` check the editor on every poll.
    const FAST: PollIntervals = PollIntervals {
        result: Duration::from_millis(1),
        liveness: Duration::ZERO,
    };

    fn result_json() -> String {
        base_result_json().to_string()
    }

    #[test]
    fn wait_for_result_returns_a_result_that_is_already_there() {
        let dir = TempDir::new("wait-present");
        let file = dir.path().join("build.json");
        fs::write(&file, result_json()).unwrap();

        let result = wait_for_result(&file, FAST, || panic!("editor should not be checked"));

        assert_eq!(result.unwrap().status, EditorBuildStatus::Success);
    }

    #[test]
    fn wait_for_result_waits_while_the_editor_runs() {
        let dir = TempDir::new("wait-running");
        let file = dir.path().join("build.json");
        let mut checks = 0;

        // The result appears after a few liveness checks.
        let result = wait_for_result(&file, FAST, || {
            checks += 1;
            if checks == 3 {
                fs::write(&file, result_json()).unwrap();
            }
            true
        });

        assert!(result.is_ok());
        assert_eq!(checks, 3);
    }

    #[test]
    fn wait_for_result_stops_when_the_editor_exits_without_a_result() {
        let dir = TempDir::new("wait-exited");
        let file = dir.path().join("build.json");

        let error = wait_for_result(&file, FAST, || false).unwrap_err();

        assert!(error.to_string().contains("no longer running"));
    }

    #[test]
    fn wait_for_result_accepts_a_result_written_just_before_the_editor_exits() {
        let dir = TempDir::new("wait-exit-after-write");
        let file = dir.path().join("build.json");

        let result = wait_for_result(&file, FAST, || {
            fs::write(&file, result_json()).unwrap();
            false
        });

        assert!(result.is_ok());
    }

    #[test]
    fn wait_for_result_retries_a_partially_written_result() {
        let dir = TempDir::new("wait-partial");
        let file = dir.path().join("build.json");
        let json = result_json();
        fs::write(&file, &json[..json.len() / 2]).unwrap();

        // An older builder script finishes writing the file in place.
        let result = wait_for_result(&file, FAST, || {
            fs::write(&file, &json).unwrap();
            true
        });

        assert!(result.is_ok());
    }

    #[test]
    fn wait_for_result_gives_up_on_a_result_that_stays_unreadable() {
        let dir = TempDir::new("wait-unreadable");
        let file = dir.path().join("build.json");
        fs::write(&file, "{ not json").unwrap();

        let error = wait_for_result(&file, FAST, || true).unwrap_err();

        assert!(
            error
                .to_string()
                .starts_with("Failed to read build result from the editor")
        );
    }
}
