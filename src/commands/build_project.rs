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

use crate::cli::{BuildArguments, BuildMode, BuildOptions, BuildTarget, InjectAction};
use crate::unity::*;

const BUILD_SCRIPT_NAME: &str = "UcomBuilder.cs";
const PERSISTENT_BUILD_SCRIPT_PATH: &str = "Assets/Plugins/ucom/Editor/UcomBuilder.cs";
const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/ucom";
const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/ucom";

pub const UNITY_BUILD_SCRIPT: &str = include_str!("include/UcomBuilder.cs");

/// Runs the build command.
pub fn build_project(arguments: BuildArguments) -> anyhow::Result<()> {
    let project_dir = validate_project_path(&arguments.project_dir)?;
    let (version, editor_exe) = editor_used_by_project(&project_dir)?;

    let output_dir = arguments.build_path.unwrap_or_else(|| {
        // If no build path is given, use <project>/Builds/<target>
        project_dir
            .join("Builds")
            .join(arguments.target.to_string())
    });

    if project_dir == output_dir {
        return Err(anyhow!(
            "Output directory cannot be the same as the project directory: {}",
            project_dir.display()
        ));
    }

    let log_file = arguments
        .log_file
        .unwrap_or_else(|| format!("Build-{}.log", arguments.target).into());
    let log_file = log_file_path(&log_file, &project_dir)?;
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
            &BuildTarget::from(arguments.target).to_string(),
        ]);

    // Combine the build option flags into an int.
    let build_options = arguments
        .build_options
        .iter()
        .fold(0, |options, &o| options | (o as i32));

    if build_options != (BuildOptions::None as i32) {
        cmd.args(["--ucom-build-options", &build_options.to_string()]);
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
        println!("{}", cmd_to_string(&cmd));
        return Ok(());
    }

    println!(
        "{}",
        format!(
            "Building Unity {version} {} project in: {}",
            arguments.target,
            project_dir.display()
        )
        .bold()
    );

    let (inject_build_script, remove_build_script) =
        new_build_script_injection_functions(&project_dir, arguments.inject);

    inject_build_script()?;

    let show_log = !arguments.quiet
        && (arguments.mode == BuildMode::Batch || arguments.mode == BuildMode::BatchNoGraphics);

    let build_result = if show_log {
        wait_with_log_output(cmd, &log_file)
    } else {
        wait_with_stdout(cmd)
    };

    remove_build_script()?;

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

    build_result.map_err(|_| errors_from_log(&log_file))
}

fn clean_output_directory(path: &Path) -> anyhow::Result<()> {
    let delete = fs::read_dir(path)?
        .flat_map(|r| r.map(|e| e.path())) // Convert to paths.
        .filter(|p| p.is_dir()) // Only directories.
        .filter(|p| {
            // Filter out directories that should not be deleted.
            p.to_string_lossy()
                .ends_with("_BurstDebugInformation_DoNotShip")
                || p.to_string_lossy()
                    .ends_with("_BackUpThisFolder_ButDontShipItWithYourGame")
        })
        .collect_vec();

    for dir in delete {
        println!("Removing directory: {}", dir.display());
        fs::remove_dir_all(&dir)
            .map_err(|_| anyhow!("Could not remove directory: {}", dir.display()))?;
    }

    Ok(())
}

/// Returns full path to the log file. By default the project's `Logs` directory is used as destination.
fn log_file_path(log_file: &Path, project_dir: &Path) -> anyhow::Result<PathBuf> {
    let Some(file_name) = log_file.file_name() else {
             return Err(anyhow!("Invalid log file name: {}", log_file.display()));
         };

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
fn errors_from_log(log_file: &Path) -> anyhow::Error {
    let Ok(log_file) = File::open(log_file) else {
        return anyhow!("Failed to open log file: {}", log_file.display());
    };

    let errors: IndexSet<_> = BufReader::new(log_file)
        .lines()
        .flatten()
        .filter(|l| is_log_error(l))
        .collect();

    match errors.len() {
        0 => anyhow!("No errors found in log"),
        1 => anyhow!("{}", errors[0]),
        _ => {
            let mut joined = String::new();
            for (i, error) in errors.iter().enumerate() {
                joined.push_str(format!("{error}: {}\n", format!("{}", i + 1).bold()).as_str());
            }
            anyhow!(joined)
        }
    }
}

/// Returns true if the given line is an error.
fn is_log_error(line: &str) -> bool {
    line.starts_with("[Builder] Error:")
        || line.contains("error CS")
        || line.starts_with("Fatal Error")
        || line.starts_with("Error building Player")
        || line.starts_with("error:")
        || line.starts_with("BuildFailedException:")
}

type ResultFn = Box<dyn FnOnce() -> anyhow::Result<()>>;

/// Creates actions that inject a script into the project before and after the build.
fn new_build_script_injection_functions(
    project_dir: &Path,
    inject: InjectAction,
) -> (ResultFn, ResultFn) {
    match (
        inject,
        project_dir.join(PERSISTENT_BUILD_SCRIPT_PATH).exists(),
    ) {
        (InjectAction::Auto, true) => {
            // Build script already present, no need to inject.
            (Box::new(|| Ok(())), Box::new(|| Ok(())))
        }

        (InjectAction::Auto, false) => {
            // Build script not present, inject it.
            // Place the build script in a unique directory to avoid conflicts.
            let uuid = Uuid::new_v4();
            let pre_root = project_dir.join(format!("{AUTO_BUILD_SCRIPT_ROOT}-{uuid}"));
            let post_root = pre_root.clone();
            (
                Box::new(|| inject_build_script(pre_root)),
                Box::new(|| remove_build_script(post_root)),
            )
        }

        (InjectAction::Persistent, true) => {
            // Build script already present, no need to inject.
            (Box::new(|| Ok(())), Box::new(|| Ok(())))
        }

        (InjectAction::Persistent, false) => {
            // Build script not present, inject it.
            let persistent_root = project_dir.join(PERSISTENT_BUILD_SCRIPT_ROOT);
            (
                Box::new(|| inject_build_script(persistent_root)),
                Box::new(|| Ok(())),
            )
        }

        (InjectAction::Off, _) => {
            // No need to do anything.
            (Box::new(|| Ok(())), Box::new(|| Ok(())))
        }
    }
}

/// Injects the build script into the project.
fn inject_build_script<P: AsRef<Path>>(parent_dir: P) -> anyhow::Result<()> {
    let inject_dir = parent_dir.as_ref().join("Editor");
    fs::create_dir_all(&inject_dir)?;

    let file_path = inject_dir.join(BUILD_SCRIPT_NAME);
    println!("Injecting ucom build script: {}", file_path.display());

    let mut file = File::create(file_path)?;
    write!(file, "{UNITY_BUILD_SCRIPT}").map_err(Into::into)
}

/// Removes the injected build script from the project.
fn remove_build_script<P: AsRef<Path>>(parent_dir: P) -> anyhow::Result<()> {
    if !parent_dir.as_ref().exists() {
        return Ok(());
    }

    println!(
        "Removing injected ucom build script: {}",
        parent_dir.as_ref().display()
    );

    // Remove the directory where the build script is located.
    fs::remove_dir_all(&parent_dir).map_err(|_| {
        anyhow!(
            "Could not remove directory: {}",
            parent_dir.as_ref().display()
        )
    })?;

    // Remove the .meta file.
    let meta_file = parent_dir.as_ref().with_extension("meta");
    if !meta_file.exists() {
        return Ok(());
    }

    fs::remove_file(&meta_file).map_err(|_| anyhow!("Could not remove: {}", meta_file.display()))
}
