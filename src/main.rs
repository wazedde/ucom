use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use std::{env, fs};

use anyhow::{anyhow, Context, Result};
use clap::CommandFactory;
use clap::Parser;
use path_absolutize::Absolutize;
use uuid::Uuid;

use crate::cli::*;
use crate::command_ext::*;
use crate::consts::*;

mod cli;
mod command_ext;
mod consts;

type OptionalFn = Option<Box<dyn FnOnce() -> Result<()>>>;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.injected_script {
        println!("{}", BUILD_SCRIPT);
        exit(0);
    }

    let Some(command) = cli.command else {
        let _ = Cli::command().print_help();
        exit(0)
    };

    match command {
        Action::List { version_pattern } => {
            list_command(version_pattern.as_deref()).context("Cannot list installations")
        }
        Action::Info { project_dir } => info_command(project_dir).context("Cannot show info"),
        Action::Run(settings) => run_command(settings).context("Cannot run Unity"),
        Action::New(settings) => new_command(settings).context("Cannot create new Unity project"),
        Action::Open(settings) => open_command(settings).context("Cannot open Unity project"),
        Action::Build(settings) => build_command(settings).context("Cannot build project"),
    }
}

/// Lists installed Unity versions.
fn list_command(partial_version: Option<&str>) -> Result<()> {
    let dir = editor_parent_dir()?;

    let versions = available_unity_versions(&dir)?;

    let default_version = env::var_os(ENV_DEFAULT_VERSION)
        .and_then(|env_version| {
            versions.iter().rev().find(|v| {
                v.to_string_lossy()
                    .starts_with(env_version.to_string_lossy().as_ref())
            })
        })
        .unwrap_or_else(|| versions.last().unwrap())
        .to_owned();

    let versions = available_matching_versions(versions, partial_version)?;

    println!(
        "List installed Unity versions in '{}'",
        dir.to_string_lossy()
    );

    for editor in versions {
        if editor == default_version {
            println!("{} (default)", editor.to_string_lossy());
        } else {
            println!("{}", editor.to_string_lossy());
        }
    }
    Ok(())
}

/// Shows project information.
fn info_command(project_dir: PathBuf) -> Result<()> {
    let project_dir = validate_project_path(&project_dir)?;
    let version = version_used_by_project(&project_dir)?;

    let availability = if editor_parent_dir()?.join(&version).exists() {
        "installed"
    } else {
        "not installed"
    };

    println!("Project info for '{}'", project_dir.to_string_lossy());
    println!("    Unity version: {} ({})", version, availability);
    Ok(())
}

/// Runs the Unity Editor with the given arguments.
fn run_command(arguments: RunArguments) -> Result<()> {
    let (version, editor_exe) = matching_editor(arguments.version_pattern.as_deref())?;

    let mut cmd = Command::new(editor_exe);
    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", cmd.to_command_line_string());
        return Ok(());
    }

    if !arguments.quiet {
        println!("Run Unity {}", version.to_string_lossy());
    }

    if arguments.wait {
        cmd.wait_with_stdout()
    } else {
        cmd.forget()
    }
}

/// Creates a new Unity project and optional Git repository in the given directory.
fn new_command(arguments: NewArguments) -> Result<()> {
    let project_dir = arguments.project_dir.absolutize()?;

    if project_dir.exists() {
        return Err(anyhow!(
            "Directory already exists: '{}'",
            project_dir.absolutize()?.to_string_lossy()
        ));
    }

    let (version, editor_exe) = matching_editor(arguments.version_pattern.as_deref())?;

    let mut cmd = Command::new(editor_exe);
    cmd.arg("-createProject")
        .arg(project_dir.as_ref())
        .args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", cmd.to_command_line_string());
        return Ok(());
    }

    if !arguments.quiet {
        println!(
            "Create new Unity {} project in '{}'",
            version.to_string_lossy(),
            project_dir.to_string_lossy()
        );
    }

    if !arguments.no_git {
        git_init(project_dir)?;
    }

    if arguments.wait {
        cmd.wait_with_stdout()?;
    } else {
        cmd.forget()?;
    }

    Ok(())
}

/// Opens the given Unity project in the Unity Editor.
fn open_command(arguments: OpenArguments) -> Result<()> {
    let project_dir = validate_project_path(&arguments.project_dir)?;

    let (version, editor_exe) = if arguments.version_pattern.is_some() {
        matching_editor(arguments.version_pattern.as_deref())?
    } else {
        matching_editor_used_by_project(&project_dir)?
    };

    // Build the command to execute.
    let mut cmd = Command::new(editor_exe);
    cmd.args(["-projectPath", &project_dir.to_string_lossy()])
        .args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", cmd.to_command_line_string());
        return Ok(());
    }

    if !arguments.quiet {
        println!(
            "Open Unity {} project in '{}'",
            version.to_string_lossy(),
            project_dir.to_string_lossy()
        );
    }

    if arguments.wait {
        cmd.wait_with_stdout()?;
    } else {
        cmd.forget()?;
    }
    Ok(())
}

/// Runs the build command.
fn build_command(arguments: BuildArguments) -> Result<()> {
    let project_dir = validate_project_path(&arguments.project_dir)?;

    let output_dir = arguments.build_path.unwrap_or_else(|| {
        project_dir
            .join("Builds")
            .join(arguments.target.to_string())
    });

    let Some(log_file) = arguments.log_file.file_name() else {
        return Err(anyhow!("Invalid log file name: {}", arguments.log_file.to_string_lossy()));
    };

    let log_file = if log_file == arguments.log_file {
        // Log filename without path was given, use the output path as destination.
        output_dir.join(log_file)
    } else {
        // A full path was given, use it.
        log_file.into()
    };

    if log_file.exists() {
        fs::remove_file(&log_file)?;
    }

    let (cmd, description) = new_build_project_command(
        &project_dir,
        arguments.target,
        &output_dir,
        arguments.mode,
        &log_file,
        arguments.args.as_deref(),
    )?;

    if arguments.dry_run {
        println!("{}", cmd.to_command_line_string());
        return Ok(());
    }

    let (pre_build, post_build) =
        create_build_script_injection_actions(&project_dir, arguments.inject);

    if let Some(pre_build) = pre_build {
        pre_build()?;
    }

    println!("{}", description);

    let build_result = match arguments.mode {
        BuildMode::Batch => cmd.wait_with_log_capture(&log_file),
        BuildMode::BatchNoGraphics => cmd.wait_with_log_capture(&log_file),
        BuildMode::EditorQuit => cmd.wait_with_stdout(),
        BuildMode::Editor => cmd.wait_with_stdout(),
    };

    if let Some(post_build) = post_build {
        post_build()?;
    }

    if build_result.is_ok() {
        println!("Build completed successfully.");
    } else {
        collect_errors_from_log(&log_file)?;
    }
    build_result
}

/// Returns errors from the given log file as one collected Err.
fn collect_errors_from_log(log_file: &PathBuf) -> Result<()> {
    let Ok(log_file) = File::open(log_file) else {
        // No log file, no errors.
        return Ok(());
    };

    let mut errors = BufReader::new(log_file)
        .lines()
        .flatten()
        .filter(|l| {
            l.starts_with("[UcomBuilder] Error:")
                || l.contains("error CS")
                || l.starts_with("Fatal Error")
        })
        .collect::<Vec<String>>();

    if errors.is_empty() {
        return Ok(());
    }

    // Remove duplicate entries
    errors.sort_unstable();
    errors.dedup();

    Err(anyhow!(errors.join("\n")))
}

/// Returns command that builds the project at the given path.
fn new_build_project_command(
    project_dir: &Path,
    build_target: Target,
    output_dir: &Path,
    mode: BuildMode,
    log_file: &Path,
    unity_args: Option<&[String]>,
) -> Result<(Command, String)> {
    if project_dir == output_dir {
        return Err(anyhow!(
            "Output directory cannot be the same as the project directory."
        ));
    }

    let (version, editor_exe) = matching_editor_used_by_project(&project_dir)?;

    // Build the command to execute.
    let mut cmd = Command::new(editor_exe);
    cmd.args(["-projectPath", &project_dir.to_string_lossy()])
        .args(["-buildTarget", &build_target.to_string()])
        .args(["-logFile", &log_file.to_string_lossy()])
        .args(["-executeMethod", "ucom.UcomBuilder.Build"])
        .args(["--ucom-build-output", &output_dir.to_string_lossy()])
        .args([
            "--ucom-build-target",
            &BuildTarget::from(build_target).to_string(),
        ]);

    // Add the build mode.
    match mode {
        BuildMode::BatchNoGraphics => {
            cmd.args(["-batchmode", "-nographics", "-quit"]);
        }
        BuildMode::Batch => {
            cmd.args(["-batchmode", "-quit"]);
        }
        BuildMode::EditorQuit => {
            cmd.args(["-quit"]);
        }
        BuildMode::Editor => {} // Do nothing.
    }

    // Add any additional arguments.
    cmd.args(unity_args.unwrap_or_default());

    Ok((
        cmd,
        format!(
            "Building Unity {} {} project in '{}' to '{}'",
            version.to_string_lossy(),
            build_target,
            project_dir.to_string_lossy(),
            output_dir.to_string_lossy(),
        ),
    ))
}

/// Creates actions that inject a script into the project before and after the build.
fn create_build_script_injection_actions(
    project_dir: &Path,
    inject: InjectAction,
) -> (OptionalFn, OptionalFn) {
    match inject {
        InjectAction::Auto => {
            if project_dir.join(PERSISTENT_BUILD_SCRIPT_PATH).exists() {
                // Build script already present, no need to inject.
                (None, None)
            } else {
                // Build script not present, inject it.
                // Place the build script in a unique directory to avoid conflicts.
                let pre_root =
                    project_dir.join(format!("{}-{}", AUTO_BUILD_SCRIPT_ROOT, Uuid::new_v4()));
                let post_root = pre_root.clone();

                (
                    Some(Box::new(|| inject_build_script(pre_root))),
                    Some(Box::new(|| remove_build_script(post_root))),
                )
            }
        }
        InjectAction::Persistent => {
            if project_dir.join(PERSISTENT_BUILD_SCRIPT_PATH).exists() {
                // Build script already present, no need to inject.
                (None, None)
            } else {
                // Build script not present, inject it.
                let persistent_root = project_dir.join(PERSISTENT_BUILD_SCRIPT_ROOT);

                (
                    Some(Box::new(|| inject_build_script(persistent_root))),
                    None,
                )
            }
        }
        InjectAction::Off => (None, None), // Do nothing.
    }
}

/// Returns the Unity version used for the project.
fn version_used_by_project<P: AsRef<Path>>(project_dir: &P) -> Result<String> {
    const PROJECT_VERSION_FILE: &str = "ProjectSettings/ProjectVersion.txt";
    let version_file = project_dir.as_ref().join(PROJECT_VERSION_FILE);

    if !version_file.exists() {
        return Err(anyhow!(
            "Directory does not contain a Unity project: '{}'",
            project_dir.as_ref().to_string_lossy()
        ));
    }

    let mut reader = BufReader::new(File::open(&version_file)?);

    // ProjectVersion.txt looks like this:
    // m_EditorVersion: 2021.3.9f1
    // m_EditorVersionWithRevision: 2021.3.9f1 (ad3870b89536)

    let mut line = String::new();
    // Read the 1st line.
    let _ = reader.read_line(&mut line)?;

    line.starts_with("m_EditorVersion:")
        .then_some(line)
        .and_then(|l| {
            l.split(':') // Split the line,
                .nth(1) // and return 2nd element.
                .map(|version| version.trim().to_owned()) // Clean it up.
        })
        .ok_or_else(|| {
            anyhow!(
                "Could not get project version from '{}'",
                version_file.to_string_lossy()
            )
        })
}

/// Returns the parent directory of the editor installations.
fn editor_parent_dir<'a>() -> Result<Cow<'a, Path>> {
    if let Some(path) = env::var_os(ENV_EDITOR_DIR) {
        let path = Path::new(&path);
        return if path.exists() && path.is_dir() {
            Ok(path.to_path_buf().into())
        } else {
            Err(anyhow!(
                "Editor directory set by {} is not a valid directory: '{}'",
                ENV_EDITOR_DIR,
                path.to_string_lossy()
            ))
        };
    }

    let path = Path::new(UNITY_EDITOR_DIR);
    if path.exists() {
        Ok(path.into())
    } else {
        Err(anyhow!(
            "Set {} to the editor directory, the default directory does not exist: '{}'",
            ENV_EDITOR_DIR,
            path.to_string_lossy()
        ))
    }
}

/// Returns list of available versions that match the partial version or Err if there is no matching version.
///
/// # Arguments
///
/// * `partial_version`: An optional partial version to match. If None, all versions are returned.
/// * `versions`: List of versions to filter.
///
fn available_matching_versions(
    versions: Vec<OsString>,
    partial_version: Option<&str>,
) -> Result<Vec<OsString>> {
    let Some(partial_version) = partial_version else {
        return Ok(versions);
    };

    let versions: Vec<_> = versions
        .into_iter()
        .filter(|v| v.to_string_lossy().starts_with(partial_version))
        .collect();

    if versions.is_empty() {
        return Err(anyhow!(
            "No Unity installation was found that matches version '{}'.",
            partial_version
        ));
    }

    Ok(versions)
}

/// Returns version and path to the editor app of the latest installed version that matches the partial version.
///
/// # Arguments
///
/// * `partial_version`: An optional partial version to match. If None, all versions are returned.
///
fn matching_editor(partial_version: Option<&str>) -> Result<(OsString, PathBuf)> {
    let parent_dir = editor_parent_dir()?;

    let version =
        available_matching_versions(available_unity_versions(&parent_dir)?, partial_version)?
            .last()
            .unwrap() // Guaranteed to have at least one entry.
            .to_owned();

    let editor_exe = parent_dir.join(&version).join(UNITY_EDITOR_EXE);
    Ok((version, editor_exe))
}

/// Returns version used by the project and the path to the editor.
fn matching_editor_used_by_project<P: AsRef<Path>>(project_dir: &P) -> Result<(OsString, PathBuf)> {
    // Get the Unity version the project uses.
    let version = version_used_by_project(project_dir)?;

    // Check if that Unity version is installed.
    let editor_dir = editor_parent_dir()?.join(&version);
    if !editor_dir.exists() {
        return Err(anyhow!(
            "Unity version that the project uses is not installed: {}",
            version
        ));
    }
    Ok((version.into(), editor_dir.join(UNITY_EDITOR_EXE)))
}

/// Returns a natural sorted list of available Unity versions.
fn available_unity_versions<P: AsRef<Path>>(install_dir: &P) -> Result<Vec<OsString>> {
    let mut versions: Vec<_> = fs::read_dir(install_dir)
        .map_err(|e| {
            anyhow!(
                "Cannot read available Unity editors in '{}': {}",
                install_dir.as_ref().to_string_lossy(),
                e
            )
        })?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| path.join(UNITY_EDITOR_EXE).exists())
        .flat_map(|path| path.file_name().map(|f| f.to_owned()))
        .collect();

    if versions.is_empty() {
        return Err(anyhow!(
            "No Unity installations found in '{}'",
            install_dir.as_ref().to_string_lossy()
        ));
    }

    versions.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
    Ok(versions)
}

/// Returns validated absolute path to the project directory.
fn validate_project_path<P: AsRef<Path>>(project_dir: &P) -> Result<Cow<Path>> {
    let path = project_dir.as_ref();
    if cfg!(target_os = "windows") && path.starts_with("~") {
        return Err(anyhow!(
            "On Windows the path cannot start with '~': '{}'",
            path.to_string_lossy()
        ));
    }

    if !path.exists() {
        return Err(anyhow!(
            "Directory does not exists: '{}'",
            path.to_string_lossy()
        ));
    }

    if !path.is_dir() {
        return Err(anyhow!(
            "Path is not a directory: '{}'",
            path.to_string_lossy()
        ));
    }

    if path.has_root() {
        return Ok(path.into());
    }

    Ok(path.absolutize()?)
}

/// Initializes a new git repository with a default Unity specific .gitignore.
fn git_init<P: AsRef<Path>>(project_dir: P) -> Result<()> {
    let project_dir = project_dir.as_ref();
    Command::new("git")
        .arg("init")
        .arg(project_dir)
        .output()
        .map_err(|_| anyhow!("Could not create git repository. Make sure git is available or add the --no-git flag."))?;

    let mut file = File::create(project_dir.join(".gitignore"))?;
    write!(file, "{}", GIT_IGNORE).map_err(|e| e.into())
}

/// Injects the build script into the project.
fn inject_build_script<P: AsRef<Path>>(parent_dir: P) -> Result<()> {
    let inject_dir = parent_dir.as_ref().join("Editor");
    fs::create_dir_all(&inject_dir)?;

    let file_path = inject_dir.join(BUILD_SCRIPT_NAME);
    println!(
        "Injecting ucom build script: {}",
        file_path.to_string_lossy()
    );

    let mut file = File::create(file_path)?;
    write!(file, "{}", BUILD_SCRIPT).map_err(|e| e.into())
}

/// Removes the injected build script from the project.
fn remove_build_script<P: AsRef<Path>>(parent_dir: P) -> Result<()> {
    if !parent_dir.as_ref().exists() {
        return Ok(());
    }

    println!(
        "Removing injected ucom build script in directory: {}",
        parent_dir.as_ref().to_string_lossy()
    );

    // Remove the directory where the build script is located.
    fs::remove_dir_all(&parent_dir).map_err(|_| {
        anyhow!(
            "Could not remove directory: '{}'",
            parent_dir.as_ref().to_string_lossy()
        )
    })?;

    // Remove the .meta file.
    let meta_file = parent_dir.as_ref().with_extension("meta");
    if !meta_file.exists() {
        return Ok(());
    }

    fs::remove_file(&meta_file)
        .map_err(|_| anyhow!("Could not remove file: '{}'", meta_file.to_string_lossy()))
}
