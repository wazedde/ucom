use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use chrono::Utc;
use colored::Colorize;
use indexmap::IndexSet;
use itertools::Itertools;
use path_absolutize::Absolutize;
use uuid::Uuid;

use crate::cli::{
    BuildArguments, BuildMode, BuildOptions, BuildScriptTarget, IncludedFile, InjectAction,
};
use crate::commands::{add_file_to_project, PERSISTENT_BUILD_SCRIPT_ROOT, time_delta_to_seconds};
use crate::unity::*;

const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/Ucom";

/// Runs the build command.
pub fn build_project(arguments: BuildArguments) -> anyhow::Result<()> {
    let start_time = Utc::now();
    let project = ProjectPath::try_from(&arguments.project_dir)?;
    let unity_version = project.unity_version()?;
    let editor_exe = unity_version.editor_executable_path()?;

    let output_dir = match &arguments.build_path {
        Some(path) => path.absolutize()?.into(),
        None => {
            // If no build path is given, use <project>/Builds/<target>
            project
                .as_path()
                .join("Builds")
                .join(arguments.output_type.to_string())
                .join(arguments.target.to_string())
        }
    };

    if project.as_path() == output_dir {
        return Err(anyhow!(
            "Output directory cannot be the same as the project directory: {}",
            project.as_path().display()
        ));
    }

    let log_file = match &arguments.log_file {
        Some(path) => path.to_owned(),
        None => format!("Build-{}.log", arguments.target).into(),
    };

    let log_file = get_full_log_path(&log_file, project.as_path())?;
    if log_file.exists() {
        fs::remove_file(&log_file)?;
    }

    // Build the command to execute.
    let mut cmd = Command::new(editor_exe);
    cmd.args(["-projectPath", &project.as_path().to_string_lossy()])
        .args(["-buildTarget", &arguments.target.to_string()])
        .args(["-logFile", &log_file.to_string_lossy()])
        .args(["-executeMethod", &arguments.build_function])
        .args(["--ucom-build-output", &output_dir.to_string_lossy()])
        .args([
            "--ucom-build-target",
            &BuildScriptTarget::from(arguments.target).to_string(),
        ]);

    let build_options = arguments.get_build_option_flags();
    if build_options != (BuildOptions::None as i32) {
        cmd.args(["--ucom-build-options", &build_options.to_string()]);
    }

    if let Some(build_args) = arguments.build_args {
        cmd.args(["--ucom-pre-build-args", &build_args]);
    }

    // Add the build mode.
    match arguments.mode {
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
    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", build_command_line(&cmd));
        return Ok(());
    }

    println!(
        "{}",
        format!(
            "Building Unity {unity_version} {} project in: {}",
            arguments.target,
            project.as_path().display()
        )
            .bold()
    );

    let (inject_build_script_hook, cleanup_build_script_hook) =
        csharp_build_script_injection_hooks(&project, arguments.inject);

    inject_build_script_hook()?;

    let show_log = !arguments.quiet
        && (arguments.mode == BuildMode::Batch || arguments.mode == BuildMode::BatchNoGraphics);

    let build_result = if show_log {
        wait_with_log_output(cmd, &log_file)
    } else {
        wait_with_stdout(cmd)
    };

    cleanup_build_script_hook()?;

    if build_result.is_ok() {
        if arguments.clean {
            clean_output_directory(&output_dir)?;
        }

        println!(
            "{} in {:.2}s",
            "Build succeeded".green().bold(),
            time_delta_to_seconds(Utc::now().signed_duration_since(start_time))
        );
    } else {
        println!(
            "{} after {:.2}s",
            "Build failed".red().bold(),
            time_delta_to_seconds(Utc::now().signed_duration_since(start_time))
        );
    }

    if let Ok(log_file) = File::open(&log_file) {
        // Iterate over lines from the build report in the log file.
        BufReader::new(log_file)
            .lines()
            .map_while(Result::ok)
            .skip_while(|l| !l.starts_with("[Builder] Build Report")) // Find marker.
            .skip(1) // Skip the marker.
            .take_while(|l| !l.is_empty()) // Read until empty line.
            .for_each(|l| println!("{l}"));
    }

    build_result.map_err(|_| collect_log_errors(&log_file))
}

impl BuildArguments {
    fn get_build_option_flags(&self) -> i32 {
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

/// Returns the full path to the log file.
/// By default, the project's `Logs` directory is used as destination.
fn get_full_log_path(log_file: &Path, project_dir: &Path) -> anyhow::Result<PathBuf> {
    let file_name = log_file
        .file_name()
        .ok_or_else(|| anyhow!("Invalid log file name: {}", log_file.display()))?;

    let path = if log_file == file_name {
        // Log filename without the path was given,
        // use the project's `Logs` directory as destination.
        project_dir.join("Logs").join(file_name)
    } else {
        log_file.into()
    };

    let path = path.absolutize()?.to_path_buf();
    Ok(path)
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

type ResultFn = Box<dyn FnOnce() -> anyhow::Result<()>>;

/// Creates actions that inject a script into the project before and after the build.
fn csharp_build_script_injection_hooks(
    project: &ProjectPath,
    inject: InjectAction,
) -> (ResultFn, ResultFn) {
    let project_path = project.as_path();

    let persistent_script_exists = project_path
        .join(PERSISTENT_BUILD_SCRIPT_ROOT)
        .join(IncludedFile::Builder.data().filename)
        .exists();
    let do_nothing: (ResultFn, ResultFn) = (Box::new(|| Ok(())), Box::new(|| Ok(())));

    match inject {
        // Build script is already present, no need to inject.
        InjectAction::Auto if persistent_script_exists => do_nothing,

        // Build script is not present, inject it in a unique directory to avoid conflicts.
        InjectAction::Auto => {
            let uuid = Uuid::new_v4();
            let unique_dir_name = format!("{AUTO_BUILD_SCRIPT_ROOT}-{uuid}");

            let closure_project_dir = project_path.to_path_buf();
            let closure_script_dir = PathBuf::from(&unique_dir_name).join("Editor");
            let closure_remove_dir = project_path.join(&unique_dir_name);

            (
                Box::new(|| {
                    add_file_to_project(
                        closure_project_dir,
                        closure_script_dir,
                        IncludedFile::Builder,
                    )
                }),
                Box::new(|| cleanup_csharp_build_script(closure_remove_dir)),
            )
        }

        // Build script is already present, no need to inject.
        InjectAction::Persistent if persistent_script_exists => do_nothing,

        // Build script is not present, inject it.
        InjectAction::Persistent => {
            let closure_project_dir = project_path.to_path_buf();
            let closure_script_dir = PathBuf::from(PERSISTENT_BUILD_SCRIPT_ROOT);

            (
                Box::new(|| {
                    add_file_to_project(
                        closure_project_dir,
                        closure_script_dir,
                        IncludedFile::Builder,
                    )
                }),
                Box::new(|| Ok(())),
            )
        }

        // No need to do anything.
        InjectAction::Off => do_nothing,
    }
}

/// Removes the injected build script from the project.
fn cleanup_csharp_build_script<P: AsRef<Path>>(parent_dir: P) -> anyhow::Result<()> {
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
