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

use crate::cli::{Action, Cli, Target};
use crate::cmd::{CmdRunner, FnCmdAction};

mod cli;
mod cmd;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        let _ = Cli::command().print_help();
        exit(0)
    };

    match command {
        Action::List { version_pattern } => {
            show_list(version_pattern.as_deref()).context("Cannot list installations")
        }

        Action::Run(run) => run_unity_cmd(run.version_pattern.as_deref(), run.args.as_deref())
            .context("Cannot run Unity")?
            .run(run.wait, run.quiet, run.dry_run),

        Action::New(new) => new_project_cmd(
            new.version_pattern.as_deref(),
            &new.path,
            new.args.as_deref(),
            new.no_git,
        )
        .context("Cannot create project")?
        .run(new.wait, new.quiet, new.dry_run),

        Action::Open(open) => open_project_cmd(
            &open.path,
            open.version_pattern.as_deref(),
            open.args.as_deref(),
        )
        .context("Cannot open project")?
        .run(open.wait, open.quiet, open.dry_run),

        Action::Build(build) => build_project_cmd(
            &build.path,
            &build.build_path,
            &build.target,
            build.args.as_deref(),
        )
        .context("Cannot build project")?
        .run(true, build.quiet, build.dry_run),
    }
}

/// Lists installed Unity versions.
///
/// # Arguments
///
/// * `partial_version`: A partial version; e.g. 2021 will list all the 2021.x.y versions you have installed on your system.
///
/// returns: Result<(), Error>
fn show_list(partial_version: Option<&str>) -> Result<()> {
    let path = installation_root_path();
    let versions = filter_versions(partial_version, available_unity_versions(&path)?);

    let Ok(versions) = versions else {
        return Err(anyhow!("No Unity installations found in {}", path.to_string_lossy()));
    };

    println!("Installed Unity versions:");
    for editor in versions {
        println!("{}", editor.to_string_lossy());
    }
    Ok(())
}

/// Returns command that runs Unity.
///
/// # Arguments
///
/// * `partial_version`: A partial version; e.g. 2021 will match the latest 2021.x.y version you have installed on your system.
/// * `unity_args`: Arguments to pass to Unity.
///
/// returns: Result<CmdRunner, Error>
fn run_unity_cmd<'a>(
    partial_version: Option<&str>,
    unity_args: Option<&[String]>,
) -> Result<CmdRunner<'a>> {
    let (unity_version, directory) = matching_unity_version(partial_version)?;

    let mut cmd = Command::new(unity_executable_path(&directory));
    cmd.args(unity_args.unwrap_or_default());

    Ok(CmdRunner::new(
        cmd,
        None,
        format!("Running Unity {}", unity_version.to_string_lossy()),
    ))
}

/// Returns command that creates an empty project at the given path.
///
/// # Arguments
///
/// * `version_pattern`: A partial version; e.g. 2021 will match the latest 2021.x.y version you have installed on your system.
/// * `project_path`: The path to the project to create.
/// * `unity_args`: Arguments to pass to Unity.
/// * `no_git`: If true, do not initialize a git repository.
///
/// returns: Result<CmdRunner, Error>
fn new_project_cmd<'a, P>(
    version_pattern: Option<&str>,
    project_path: &P,
    unity_args: Option<&[String]>,
    no_git: bool,
) -> Result<CmdRunner<'a>>
where
    P: AsRef<Path>,
{
    let project_path = project_path.as_ref();

    // Check if destination already exists.
    if project_path.exists() {
        return Err(anyhow!(
            "Directory already exists: '{}'",
            project_path.absolutize()?.to_string_lossy()
        ));
    }

    // Create closure that initializes git repository.
    let pre_action: Option<Box<FnCmdAction>> = if !no_git {
        let p = project_path.to_owned();
        Some(Box::new(move || git_init(&p)))
    } else {
        None
    };

    let (version, unity_directory) = matching_unity_version(version_pattern)?;

    let mut cmd = Command::new(unity_executable_path(&unity_directory));
    cmd.arg("-createProject")
        .arg(project_path)
        .args(unity_args.unwrap_or_default());

    Ok(CmdRunner::new(
        cmd,
        pre_action,
        format!(
            "Creating Unity {} project in '{}'",
            version.to_string_lossy(),
            project_path.to_string_lossy()
        ),
    ))
}

/// Returns command that opens the project at the given path.
///
/// # Arguments
///
/// * `project_path`: The path to the project to open.
/// * `partial_version`: A partial version; e.g. 2021 will match the latest 2021.x.y version you have installed on your system.
/// * `unity_args`: Arguments to pass to Unity.
///
/// returns: Result<CmdRunner, Error>
fn open_project_cmd<'a, P>(
    project_path: &P,
    partial_version: Option<&str>,
    unity_args: Option<&[String]>,
) -> Result<CmdRunner<'a>>
where
    P: AsRef<Path>,
{
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
        format!(
            "Opening Unity {} project in '{}'",
            version.to_string_lossy(),
            project_path.to_string_lossy()
        ),
    ))
}

fn build_project_cmd<'a, P>(
    project_path: &P,
    output_path: &P,
    build_target: &Target,
    unity_args: Option<&[String]>,
) -> Result<CmdRunner<'a>>
where
    P: AsRef<Path>,
{
    // Make sure the project path exists and is formatted correctly.
    let project_path = validate_project_path(&project_path)?;
    let output_path = Path::new(output_path.as_ref()).absolutize()?;

    let (version, unity_directory) = matching_unity_project_version(&project_path)?;

    let unity_path = unity_executable_path(&unity_directory);

    // Build the command to execute.
    let mut cmd = Command::new(unity_path);
    cmd.args(["-projectPath", &project_path.to_string_lossy()])
        .arg("-batchmode")
        .arg("-quit")
        .args(["-buildTarget", &build_target.to_string()])
        .args(["-executeMethod", "ucom.UcomBuilder.Build"])
        .args(["--ucom-build-output", &output_path.to_string_lossy()])
        .args(unity_args.unwrap_or_default());

    Ok(CmdRunner::new(
        cmd,
        None,
        format!(
            "Building Unity {} project in '{}'",
            version.to_string_lossy(),
            project_path.to_string_lossy()
        ),
    ))
}

/// Returns the Unity version used for the project.
///
/// # Arguments
///
/// * `path`: Path to the project.
///
/// returns: Result<String, Error>
fn unity_project_version<P>(path: &P) -> Result<String>
where
    P: AsRef<Path>,
{
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
///
/// * `path`: Path to the Unity installation directory.
///
/// returns: PathBuf
fn unity_executable_path<P>(path: &P) -> PathBuf
where
    P: AsRef<Path>,
{
    if cfg!(target_os = "macos") {
        Path::new(path.as_ref()).join("Unity.app/Contents/MacOS/Unity")
    } else if cfg!(target_os = "windows") {
        Path::new(path.as_ref()).join(r"Editor\Unity.exe")
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
/// returns: Result<Vec<OsString, Global>, Error>
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
/// returns: Result<(OsString, PathBuf), Error>
fn matching_unity_version(partial_version: Option<&str>) -> Result<(OsString, PathBuf)> {
    let path = installation_root_path();

    let version = filter_versions(partial_version, available_unity_versions(&path)?)?
        .last()
        .map(|latest| latest.to_owned())
        .unwrap(); // Guaranteed to have at least one entry.

    let full_path = Path::new(&path).join(&version);
    Ok((version, full_path))
}

/// Returns version and directory for the project.
///
/// # Arguments
///
/// * `path`: Path to the project.
///
/// returns: Result<(OsString, PathBuf), Error>
fn matching_unity_project_version<P>(path: &P) -> Result<(OsString, PathBuf)>
where
    P: AsRef<Path>,
{
    // Get the Unity version the project uses.
    let version: OsString = unity_project_version(path)?.into();

    // Check if that Unity version is installed.
    let directory = Path::new(&installation_root_path()).join(&version);
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
/// returns: Result<Vec<OsString, Global>, Error>
fn available_unity_versions<P>(path: &P) -> Result<Vec<OsString>>
where
    P: AsRef<Path>,
{
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
/// returns: Result<Cow<Path>, Error>
fn validate_project_path<P>(path: &P) -> Result<Cow<Path>>
where
    P: AsRef<Path>,
{
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
/// returns: Result<(), Error>
pub fn git_init<P>(path: &P) -> Result<()>
where
    P: AsRef<Path>,
{
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
