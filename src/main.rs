use std::borrow::Cow;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

use anyhow::{anyhow, Context, Result};
use clap::CommandFactory;
use clap::Parser;
use path_absolutize::Absolutize;
use uuid::Uuid;

use crate::cli::*;
use crate::cmd::*;

mod cli;
mod cmd;

const BUILD_SCRIPT_NAME: &str = "UcomBuilder.cs";
const PERSISTENT_BUILD_SCRIPT_PATH: &str = "Assets/Plugins/ucom/Editor/UcomBuilder.cs";
const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/ucom";
const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/ucom";

type OptionalFn = Option<Box<dyn FnOnce() -> Result<()>>>;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        let _ = Cli::command().print_help();
        exit(0)
    };

    match command {
        Action::List { version_pattern } => {
            run_list_command(version_pattern.as_deref()).context("Cannot list installations")
        }

        Action::Run(run) => run_unity_command(run).context("Cannot run Unity"),

        Action::New(new) => run_new_command(new).context("Cannot create new Unity project"),

        Action::Open(open) => open_project_cmd(
            open.project_dir,
            open.version_pattern.as_deref(),
            open.args.as_deref(),
        )
        .context("Cannot open project")?
        .run(open.wait, open.quiet, open.dry_run)
        .context("Cannot open project"),

        Action::Build(build) => run_build_command(build).context("Cannot build project"),
    }
}

/// Lists installed Unity versions.
fn run_list_command(partial_version: Option<&str>) -> Result<()> {
    let path = installation_root_path();
    let versions = filter_versions(partial_version, available_unity_versions(&path)?);

    let Ok(versions) = versions else {
        return Err(anyhow!("No Unity installations found in {}", path.to_string_lossy()));
    };

    println!("List Unity versions");
    for editor in versions {
        println!("{}", editor.to_string_lossy());
    }
    Ok(())
}

/// Runs the Unity Editor with the given arguments.
fn run_unity_command(settings: Run) -> Result<()> {
    let (version, directory) = matching_unity_version(settings.version_pattern.as_deref())?;

    let mut cmd = Command::new(unity_executable_path(&directory));
    cmd.args(settings.args.unwrap_or_default());

    if settings.dry_run {
        println!("{}", to_command_line_string(&cmd));
        return Ok(());
    }

    if !settings.quiet {
        println!("Run Unity {}", version.to_string_lossy());
    }

    if settings.wait {
        run_command_to_stdout(cmd)
    } else {
        forget_command(cmd)
    }
}

/// Creates a new Unity project and optional Git repository in the given directory.
fn run_new_command(settings: New) -> Result<()> {
    // Check if destination already exists.
    if settings.project_dir.exists() {
        return Err(anyhow!(
            "Directory already exists: '{}'",
            settings.project_dir.absolutize()?.to_string_lossy()
        ));
    }

    let (version, unity_directory) = matching_unity_version(settings.version_pattern.as_deref())?;

    let mut cmd = Command::new(unity_executable_path(&unity_directory));
    cmd.arg("-createProject")
        .arg(&settings.project_dir)
        .args(settings.args.unwrap_or_default());

    if settings.dry_run {
        println!("{}", to_command_line_string(&cmd));
        return Ok(());
    }

    if !settings.quiet {
        println!(
            "Create new Unity {} project in '{}'",
            version.to_string_lossy(),
            settings.project_dir.to_string_lossy()
        );
    }

    if !settings.no_git {
        git_init(settings.project_dir)?;
    }

    if settings.wait {
        run_command_to_stdout(cmd)?;
    } else {
        forget_command(cmd).map(|_| ())?;
    }

    Ok(())
}

/// Returns command that opens the project at the given path.
fn open_project_cmd<P: AsRef<Path>>(
    project_path: P,
    partial_version: Option<&str>,
    unity_args: Option<&[String]>,
) -> Result<CmdRunner> {
    // Make sure the project path exists and is formatted correctly.
    let project_path = validate_project_path(&project_path)?;

    let (version, unity_directory) = if partial_version.is_some() {
        matching_unity_version(partial_version)?
    } else {
        matching_unity_project_version(&project_path)?
    };

    let unity_path = unity_executable_path(&unity_directory);

    // Build the command to execute.
    let mut cmd = Command::new(unity_path);
    cmd.args(["-projectPath", &project_path.to_string_lossy()])
        .args(unity_args.unwrap_or_default());

    Ok(CmdRunner::new(
        cmd,
        None,
        None,
        format!(
            "Opening Unity {} project in '{}'",
            version.to_string_lossy(),
            project_path.to_string_lossy()
        ),
    ))
}

/// Runs the build command.
fn run_build_command(settings: Build) -> Result<()> {
    let project_path = validate_project_path(&settings.project_dir)?;

    let output_path = settings.build_path.unwrap_or_else(|| {
        project_path
            .join("Builds")
            .join(settings.target.to_string())
    });

    let Some(log_file) = settings.log_file.file_name() else {
        return Err(anyhow!("Invalid log file name: {}", settings.log_file.to_string_lossy()));
    };

    let log_file = if log_file == settings.log_file {
        // Log filename without path was given, use the output path as destination.
        output_path.join(log_file)
    } else {
        // A full path was given, use it.
        log_file.into()
    };

    if log_file.exists() {
        fs::remove_file(&log_file)?;
    }

    let (command, description) = new_build_project_command(
        &project_path,
        settings.target,
        &output_path,
        settings.mode,
        &log_file,
        settings.args.as_deref(),
    )?;

    if settings.dry_run {
        println!("{}", to_command_line_string(&command));
        return Ok(());
    }

    let (pre_build, post_build) =
        create_build_script_injection_actions(&project_path, settings.inject);

    if let Some(pre_build) = pre_build {
        pre_build()?;
    }

    println!("{}", description);

    let result = if settings.mode == BuildMode::Batch || settings.mode == BuildMode::BatchNoGraphics
    {
        run_command_with_log_capture(command, &log_file)
    } else {
        run_command_to_stdout(command)
    };

    if let Some(post_build) = post_build {
        post_build()?;
    }

    result
}

/// Returns command that builds the project at the given path.
fn new_build_project_command(
    project_path: &Path,
    build_target: Target,
    output_path: &Path,
    mode: BuildMode,
    log_file: &Path,
    unity_args: Option<&[String]>,
) -> Result<(Command, String)> {
    if project_path == output_path {
        return Err(anyhow!(
            "Output path cannot be the same as the project path."
        ));
    }

    let (version, unity_directory) = matching_unity_project_version(&project_path)?;
    let unity_path = unity_executable_path(&unity_directory);

    // Build the command to execute.
    let mut cmd = Command::new(unity_path);
    cmd.args(["-projectPath", &project_path.to_string_lossy()])
        .args(["-buildTarget", &build_target.to_string()])
        .args(["-logFile", &log_file.to_string_lossy()])
        .args(["-executeMethod", "ucom.UcomBuilder.Build"])
        .args(["--ucom-build-output", &output_path.to_string_lossy()])
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
        BuildMode::Debug => {} // Do nothing.
    }

    // Add any additional arguments.
    cmd.args(unity_args.unwrap_or_default());

    Ok((
        cmd,
        format!(
            "Building Unity {} {} project in '{}' to '{}'",
            version.to_string_lossy(),
            build_target,
            project_path.to_string_lossy(),
            output_path.to_string_lossy(),
        ),
    ))
}

/// Creates actions that inject a script into the project before and after the build.
fn create_build_script_injection_actions(
    project_path: &Path,
    inject: InjectAction,
) -> (OptionalFn, OptionalFn) {
    match inject {
        InjectAction::Auto => {
            if project_path.join(PERSISTENT_BUILD_SCRIPT_PATH).exists() {
                // Build script already present, no need to inject.
                (None, None)
            } else {
                // Build script not present, inject it.
                // Place the build script in a unique directory to avoid conflicts.
                let pre_root =
                    project_path.join(format!("{}-{}", AUTO_BUILD_SCRIPT_ROOT, Uuid::new_v4()));
                let post_root = pre_root.clone();

                // Closure that injects build script into project.
                let pre_action: OptionalFn = Some(Box::new(|| inject_build_script(pre_root)));

                // Closure that removes build script.
                let post_action: OptionalFn = Some(Box::new(|| remove_build_script(post_root)));
                (pre_action, post_action)
            }
        }
        InjectAction::Persistent => {
            if project_path.join(PERSISTENT_BUILD_SCRIPT_PATH).exists() {
                // Build script already present, no need to inject.
                (None, None)
            } else {
                // Build script not present, inject it.
                let persistent_root = project_path.join(PERSISTENT_BUILD_SCRIPT_ROOT);

                // Closure that injects build script into project.
                let pre_action: OptionalFn =
                    Some(Box::new(|| inject_build_script(persistent_root)));

                (pre_action, None)
            }
        }
        InjectAction::Off => (None, None), // Do nothing.
    }
}

/// Returns the Unity version used for the project.
///
/// # Arguments
///
/// * `path`: Path to the project.
///
fn unity_project_version<P: AsRef<Path>>(path: &P) -> Result<String> {
    const PROJECT_VERSION_FILE: &str = "ProjectSettings/ProjectVersion.txt";
    let file_path = path.as_ref().join(PROJECT_VERSION_FILE);

    if !file_path.exists() {
        return Err(anyhow!(
            "Directory does not contain a Unity project: '{}'",
            path.as_ref().to_string_lossy()
        ));
    }

    let mut reader = BufReader::new(File::open(&file_path)?);

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
                "Could not get project version from: '{}'",
                file_path.to_string_lossy()
            )
        })
}

/// Returns the root path of the installations.
fn installation_root_path<'a>() -> &'a Path {
    if cfg!(target_os = "macos") {
        Path::new("/Applications/Unity/Hub/Editor/")
    } else if cfg!(target_os = "windows") {
        Path::new(r"C:\Program Files\Unity\Hub\Editor")
    } else {
        unimplemented!()
    }
}

/// Returns the path to the executable.
///
/// # Arguments
/// * `path`: Path to the Unity installation directory.
///
fn unity_executable_path<P: AsRef<Path>>(path: &P) -> PathBuf {
    if cfg!(target_os = "macos") {
        path.as_ref().join("Unity.app/Contents/MacOS/Unity")
    } else if cfg!(target_os = "windows") {
        path.as_ref().join(r"Editor\Unity.exe")
    } else {
        unimplemented!()
    }
}

/// Returns list of available versions that match the partial version or Err if there is no matching version.
///
/// # Arguments
///
/// * `partial_version`: An optional partial version to match. If None, all versions are returned.
/// * `versions`: List of versions to filter.
///
fn filter_versions(
    partial_version: Option<&str>,
    versions: Vec<OsString>,
) -> Result<Vec<OsString>> {
    let Some(pattern) = partial_version else {
        return Ok(versions);
    };

    let versions: Vec<_> = versions
        .into_iter()
        .filter(|v| v.to_string_lossy().starts_with(pattern))
        .collect();

    if versions.is_empty() {
        return Err(anyhow!(
            "No Unity installation was found that matches version {}",
            partial_version.unwrap_or("<any>")
        ));
    }

    Ok(versions)
}

/// Returns version and directory of the latest installed version that matches the partial version.
///
/// # Arguments
///
/// * `partial_version`: An optional partial version to match. If None, all versions are returned.
///
fn matching_unity_version(partial_version: Option<&str>) -> Result<(OsString, PathBuf)> {
    let path = installation_root_path();

    let version = filter_versions(partial_version, available_unity_versions(&path)?)?
        .last()
        .map(|latest| latest.to_owned())
        .unwrap(); // Guaranteed to have at least one entry.

    let full_path = path.join(&version);
    Ok((version, full_path))
}

/// Returns version and directory for the project.
///
/// # Arguments
///
/// * `path`: Path to the project.
///
fn matching_unity_project_version<P: AsRef<Path>>(path: &P) -> Result<(OsString, PathBuf)> {
    // Get the Unity version the project uses.
    let version: OsString = unity_project_version(path)?.into();

    // Check if that Unity version is installed.
    let directory = installation_root_path().join(&version);
    if !directory.exists() {
        return Err(anyhow!(
            "Unity version that the project uses is not installed: {}",
            version.to_string_lossy()
        ));
    }
    Ok((version, directory))
}

/// Returns a natural sorted list of available Unity versions.
///
/// # Arguments
///
/// * `path`: Path to the Unity installation root.
///
fn available_unity_versions<P: AsRef<Path>>(path: &P) -> Result<Vec<OsString>> {
    let mut versions: Vec<_> = fs::read_dir(path)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .flat_map(|path| path.file_name().map(|f| f.to_owned()))
        .collect();

    if versions.is_empty() {
        return Err(anyhow!(
            "No Unity installations found in {}",
            path.as_ref().to_string_lossy()
        ));
    }

    versions.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));
    Ok(versions)
}

/// Returns validated absolute path to the project.
///
/// # Arguments
///
/// * `path`: Path to validate.
///
fn validate_project_path<P: AsRef<Path>>(path: &P) -> Result<Cow<Path>> {
    let path = path.as_ref();
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
///
/// # Arguments
///
/// * `path`: Path to the project.
///
fn git_init<P: AsRef<Path>>(path: P) -> Result<()> {
    Command::new("git")
        .arg("init")
        .arg(path.as_ref())
        .output()
        .map_err(|_| anyhow!("Could not create git repository. Make sure git is available or add the --no-git flag."))?;

    let file_path = path.as_ref().join(".gitignore");
    let file_content = include_str!("include/unity-gitignore.txt");
    let mut file = File::create(file_path)?;
    write!(file, "{}", file_content)?;
    Ok(())
}

fn inject_build_script<P: AsRef<Path>>(root_path: P) -> Result<()> {
    let root_path = root_path.as_ref().join("Editor");
    fs::create_dir_all(&root_path)?;

    let file_path = root_path.join(BUILD_SCRIPT_NAME);
    println!(
        "[ucom] Injecting build script: {}",
        file_path.to_string_lossy()
    );

    let file_content = include_str!("include/UcomBuilder.cs");
    let mut file = File::create(file_path)?;
    write!(file, "{}", file_content)?;
    Ok(())
}

/// Removes the injected build script from the project.
fn remove_build_script<P: AsRef<Path>>(root_directory: P) -> Result<()> {
    // Remove the directory where the build script is located.
    fs::remove_dir_all(&root_directory).map_err(|_| {
        anyhow!(
            "Could not remove directory: '{}'",
            root_directory.as_ref().to_string_lossy()
        )
    })?;

    println!(
        "[ucom] Removing injected build script in: {}",
        root_directory.as_ref().to_string_lossy()
    );

    // Remove the .meta file.
    let meta_file = format!("{}.meta", root_directory.as_ref().to_string_lossy());
    fs::remove_file(&meta_file).map_err(|_| anyhow!("Could not remove file: '{}'", meta_file))?;
    Ok(())
}
