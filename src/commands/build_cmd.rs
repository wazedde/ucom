use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use chrono::Utc;
use indexmap::IndexSet;
use itertools::Itertools;
use path_absolutize::Absolutize;
use uuid::Uuid;

use crate::cli_add::UnityTemplateFile;
use crate::cli_build::{BuildArguments, BuildMode, BuildOptions, BuildScriptTarget, InjectAction};
use crate::commands::{PERSISTENT_BUILD_SCRIPT_ROOT, TimeDeltaExt, add_file_to_project};
use crate::unity::{ProjectPath, build_command_line, wait_with_log_output, wait_with_stdout};
use crate::utils::status_line::{MessageType, StatusLine};

const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/Ucom";

/// Runs the build command.
pub fn build_project(arguments: &BuildArguments) -> anyhow::Result<()> {
    let start_time = Utc::now();
    let project = ProjectPath::try_from(&arguments.project_dir)?;
    let unity_version = project.unity_version()?;
    let editor_path = unity_version.editor_executable_path()?;

    let output_path = arguments.output_path(&project)?;
    let log_path = arguments.full_log_path(&project)?;

    let build_command = arguments.create_cmd(&project, &editor_path, &output_path, &log_path);

    if arguments.dry_run {
        println!("{}", build_command_line(&build_command));
        return Ok(());
    }
    let build_text = format!(
        "Unity {unity_version} {} project in {}",
        arguments.target,
        project.display()
    );

    let build_status = if arguments.quiet {
        StatusLine::new("Building", &build_text)
    } else {
        MessageType::print_line("Building", &build_text, MessageType::Info);
        StatusLine::new_silent()
    };

    let hooks = csharp_build_script_injection_hooks(&project, arguments.inject);

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
        &format!(
            "building Unity {unity_version} {} project in {}",
            arguments.target,
            project.display()
        ),
        build_status,
    );

    MessageType::print_line(
        "Total time",
        &format!(
            "{:.2}s",
            Utc::now().signed_duration_since(start_time).as_seconds()
        ),
        build_status,
    );

    print_build_report(&log_path, build_status);
    build_result.map_err(|_| collect_log_errors(&log_path))
}

impl BuildArguments {
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
            .ok_or_else(|| anyhow!("Invalid log file name: {}", log_file.display()))?;

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
                project.display()
            ));
        }
        Ok(output_dir)
    }
    fn create_cmd(
        &self,
        project: &ProjectPath,
        editor_exe: &Path,
        output_dir: &Path,
        log_file: &Path,
    ) -> Command {
        // Build the command to execute.
        let mut cmd = Command::new(editor_exe);
        cmd.args(["-projectPath", &project.to_string_lossy()])
            .args(["-buildTarget", self.target.as_ref()])
            .args(["-logFile", &log_file.to_string_lossy()])
            .args(["-executeMethod", &self.build_function])
            .args(["--ucom-build-output", &output_dir.to_string_lossy()])
            .args([
                "--ucom-build-target",
                BuildScriptTarget::from(self.target).as_ref(),
            ]);

        let build_options = self.build_option_flags();
        if build_options != (BuildOptions::None as i32) {
            cmd.args(["--ucom-build-options", &build_options.to_string()]);
        }

        if let Some(build_args) = &self.build_args {
            cmd.args(["--ucom-pre-build-args", build_args]);
        }

        // Add the build mode.
        match self.mode {
            BuildMode::BatchNoGraphics => {
                cmd.args(["-batchmode", "-nographics", "-quit"]);
            }
            BuildMode::Batch => {
                cmd.args(["-batchmode", "-quit"]);
            }
            BuildMode::EditorQuit => {
                cmd.args(["-quit"]);
            }
            BuildMode::Editor => (), // Do nothing.
        }

        // Add any additional arguments.
        if let Some(a) = self.args.as_ref() {
            cmd.args(a);
        }
        cmd
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
        .map_while(Result::ok)
        .map(|de| de.path())
        .filter(|p| p.is_dir()) // Only directories.
        .filter(|p| {
            let dir_str = p.to_string_lossy();
            dir_str.ends_with("_BurstDebugInformation_DoNotShip")
                || dir_str.ends_with("_BackUpThisFolder_ButDontShipItWithYourGame")
        });

    for dir in to_delete {
        println!("Removing directory: {}", dir.display());
        fs::remove_dir_all(&dir)
            .map_err(|_| anyhow!("Could not remove directory: {}", dir.display()))?;
    }

    Ok(())
}

/// Returns errors from the given log file as one collected Err.
fn collect_log_errors(log_file: &Path) -> anyhow::Error {
    let Ok(log_file) = File::open(log_file) else {
        return anyhow!("Failed to open log file: {}", log_file.display());
    };

    let errors: IndexSet<_> = BufReader::new(log_file)
        .lines()
        .map_while(Result::ok)
        .filter(|l| line_contains_error(l))
        .unique()
        .collect();

    match errors.len() {
        0 => anyhow!("No errors found in log"),
        1 => anyhow!("{}", errors[0]),
        _ => {
            let joined = errors
                .iter()
                .enumerate()
                .fold(String::new(), |mut output, (i, error)| {
                    output += &format!("{}: {}\n", i + 1, error);
                    output
                });
            anyhow!(joined)
        }
    }
}

/// Returns true if the given line is an error.
fn line_contains_error(line: &str) -> bool {
    let error_prefixes = &[
        "[Builder] Error:",
        "error CS",
        "Fatal Error",
        "Error building Player",
        "error:",
        "BuildFailedException:",
    ];

    error_prefixes.iter().any(|prefix| line.contains(prefix))
}

type HookFn = Box<dyn FnOnce() -> anyhow::Result<()>>;

fn no_op_hook() -> HookFn {
    Box::new(|| Ok(()))
}

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
        InjectAction::Auto if persistent_script_exists => BuildHooks::no_op(),

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

        // Build script is already present, no need to inject.
        InjectAction::Persistent if persistent_script_exists => BuildHooks::no_op(),

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

    println!(
        "Removing temporary ucom build script: {}",
        parent_dir.display()
    );

    // Remove the directory where the build script is located.
    fs::remove_dir_all(parent_dir)
        .map_err(|_| anyhow!("Could not remove directory: {}", parent_dir.display()))?;

    // Remove the .meta file.
    let meta_file = parent_dir.with_extension("meta");
    if meta_file.exists() {
        fs::remove_file(&meta_file)
            .map_err(|_| anyhow!("Could not remove: {}", meta_file.display()))?;
    }

    Ok(())
}
