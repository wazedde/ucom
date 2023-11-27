use std::fs;
use std::fs::File;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use colored::Colorize;
use indexmap::IndexSet;
use itertools::Itertools;
use path_absolutize::Absolutize;
use uuid::Uuid;

use crate::cli::{
    BuildArguments, BuildMode, BuildOptions, BuildScriptTarget, InjectAction, Template,
};
use crate::unity::*;

const BUILD_SCRIPT_NAME: &str = "UnityBuilder.cs";
const PERSISTENT_BUILD_SCRIPT_PATH: &str = "Assets/Plugins/Ucom/Editor/UnityBuilder.cs";
const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/Ucom";
const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/Ucom";

/// Runs the build command.
pub fn build_project(arguments: BuildArguments) -> anyhow::Result<()> {
    let project_dir = validate_directory(&arguments.project_dir)?;

    let unity_version = determine_unity_version(&project_dir)?;
    let editor_exe = editor_executable_path(unity_version)?;

    let output_dir = match arguments.build_path {
        Some(path) => path.absolutize()?.into(),
        None => {
            // If no build path is given, use <project>/Builds/<target>
            project_dir
                .join("Builds")
                .join(arguments.output_type.to_string())
                .join(arguments.target.to_string())
        }
    };

    if project_dir == output_dir {
        return Err(anyhow!(
            "Output directory cannot be the same as the project directory: {}",
            project_dir.display()
        ));
    }

    let log_file = arguments
        .log_file
        .unwrap_or_else(|| format!("Build-{}.log", arguments.target).into());
    let log_file = get_full_log_path(&log_file, &project_dir)?;
    if log_file.exists() {
        fs::remove_file(&log_file)?;
    }

    // Build the command to execute.
    let mut cmd = Command::new(editor_exe);
    cmd.args(["-projectPath", &project_dir.to_string_lossy()])
        .args(["-buildTarget", &arguments.target.to_string()])
        .args(["-logFile", &log_file.to_string_lossy()])
        .args(["-executeMethod", &arguments.build_function])
        .args(["--ucom-build-output", &output_dir.to_string_lossy()])
        .args([
            "--ucom-build-target",
            &BuildScriptTarget::from(arguments.target).to_string(),
        ]);

    let mut bo = arguments.build_options;

    if arguments.run_player {
        bo.push(BuildOptions::AutoRunPlayer);
    }

    if arguments.development_build {
        bo.push(BuildOptions::Development);
    }

    if arguments.show_built_player {
        bo.push(BuildOptions::ShowBuiltPlayer);
    }

    if arguments.allow_debugging {
        bo.push(BuildOptions::AllowDebugging);
    }

    if arguments.connect_with_profiler {
        bo.push(BuildOptions::ConnectWithProfiler);
    }

    if arguments.deep_profiling {
        bo.push(BuildOptions::EnableDeepProfilingSupport);
    }

    if arguments.connect_to_host {
        bo.push(BuildOptions::ConnectToHost);
    }

    // Combine the build option flags into an int.
    let build_options = bo.iter().fold(0, |options, &o| options | (o as i32));

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
            project_dir.display()
        )
        .bold()
    );

    let (inject_build_script_hook, cleanup_build_script_hook) =
        csharp_build_script_injection_hooks(&project_dir, arguments.inject);

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

        println!("{}", "Build succeeded".green().bold());
    } else {
        println!("{}", "Build failed".red().bold());
    }

    if let Ok(log_file) = File::open(&log_file) {
        // Iterate over lines from the build report in the log file.
        BufReader::new(log_file)
            .lines()
            .flatten()
            .skip_while(|l| !l.starts_with("[Builder] Build Report")) // Find marker.
            .skip(1) // Skip the marker.
            .take_while(|l| !l.is_empty()) // Read until empty line.
            .for_each(|l| println!("{l}"));
    }

    build_result.map_err(|_| collect_log_errors(&log_file))
}

/// Injects a persistent C# build script into the project.
pub fn inject_persistent_csharp_build_script<P: AsRef<Path>>(project_dir: P) -> anyhow::Result<()> {
    let persistent_root = project_dir.as_ref().join(PERSISTENT_BUILD_SCRIPT_ROOT);
    return inject_csharp_build_script(persistent_root);
}

fn clean_output_directory(path: &Path) -> anyhow::Result<()> {
    let delete = fs::read_dir(path)?
        .filter_map(|r| r.ok().map(|e| e.path()))
        .filter(|p| p.is_dir()) // Only directories.
        .filter(|p| {
            let dir_str = p.to_string_lossy();
            dir_str.ends_with("_BurstDebugInformation_DoNotShip")
                || dir_str.ends_with("_BackUpThisFolder_ButDontShipItWithYourGame")
        });

    for dir in delete {
        println!("Removing directory: {}", dir.display());
        fs::remove_dir_all(&dir)
            .map_err(|_| anyhow!("Could not remove directory: {}", dir.display()))?;
    }

    Ok(())
}

/// Returns full path to the log file. By default the project's `Logs` directory is used as destination.
fn get_full_log_path(log_file: &Path, project_dir: &Path) -> anyhow::Result<PathBuf> {
    let file_name = log_file
        .file_name()
        .ok_or_else(|| anyhow!("Invalid log file name: {}", log_file.display()))?;

    let path = if log_file == file_name {
        // Log filename without path was given, use the project's `Logs` directory as destination.
        project_dir.join("Logs").join(file_name)
    } else {
        log_file.into()
    }
    .absolutize()?
    .to_path_buf();

    Ok(path)
}

/// Returns errors from the given log file as one collected Err.
fn collect_log_errors(log_file: &Path) -> anyhow::Error {
    let Ok(log_file) = File::open(log_file) else {
        return anyhow!("Failed to open log file: {}", log_file.display());
    };

    let errors: IndexSet<_> = BufReader::new(log_file)
        .lines()
        .flatten()
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
    project_dir: &Path,
    inject: InjectAction,
) -> (ResultFn, ResultFn) {
    let script_exists = project_dir.join(PERSISTENT_BUILD_SCRIPT_PATH).exists();
    let do_nothing: (ResultFn, ResultFn) = (Box::new(|| Ok(())), Box::new(|| Ok(())));

    match inject {
        // Build script already present, no need to inject.
        InjectAction::Auto if script_exists => do_nothing,

        // Build script not present, inject it in a unique directory to avoid conflicts.
        InjectAction::Auto => {
            let uuid = Uuid::new_v4();
            let pre_root = project_dir.join(format!("{AUTO_BUILD_SCRIPT_ROOT}-{uuid}"));
            let post_root = pre_root.clone();
            (
                Box::new(|| inject_csharp_build_script(pre_root)),
                Box::new(|| cleanup_csharp_build_script(post_root)),
            )
        }

        // Build script already present, no need to inject.
        InjectAction::Persistent if script_exists => do_nothing,

        // Build script not present, inject it.
        InjectAction::Persistent => {
            let project_dir = project_dir.to_owned();
            (
                Box::new(|| inject_persistent_csharp_build_script(project_dir)),
                Box::new(|| Ok(())),
            )
        }

        // No need to do anything.
        InjectAction::Off => do_nothing,
    }
}
/// Injects the build script into the project.
fn inject_csharp_build_script<P: AsRef<Path>>(parent_dir: P) -> anyhow::Result<()> {
    let inject_dir = parent_dir.as_ref().join("Editor");
    fs::create_dir_all(&inject_dir)?;

    let file_path = inject_dir.join(BUILD_SCRIPT_NAME);
    println!("Injecting ucom build script: {}", file_path.display());

    let mut file = File::create(file_path)?;
    write!(file, "{}", Template::BuildScript.content()).map_err(Into::into)
}

/// Removes the injected build script from the project.
fn cleanup_csharp_build_script<P: AsRef<Path>>(parent_dir: P) -> anyhow::Result<()> {
    let parent_dir = parent_dir.as_ref();
    if !parent_dir.exists() {
        return Ok(());
    }

    println!(
        "Removing injected ucom build script: {}",
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
