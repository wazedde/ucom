use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

use anyhow::{anyhow, Context, Result};
use clap::CommandFactory;
use clap::Parser;

use crate::cli::Cli;
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
        cli::Action::List { version_pattern } => {
            show_list(&version_pattern).context("Cannot list installations")
        }

        cli::Action::Run(run) => run_unity_cmd(&run.version_pattern, &run.args)
            .context("Cannot run Unity")?
            .run(run.wait, run.quiet, run.dry_run),

        cli::Action::New(new) => {
            new_project_cmd(&new.version_pattern, &new.path, &new.args, new.no_git)
                .context("Cannot create project")?
                .run(new.wait, new.quiet, new.dry_run)
        }

        cli::Action::Open(open) => open_project_cmd(&open.path, &open.version_pattern, &open.args)
            .context("Cannot open project")?
            .run(open.wait, open.quiet, open.dry_run),
    }
}

/// Lists installed Unity versions.
fn show_list(partial_version: &Option<String>) -> Result<()> {
    let path = installation_root_path();
    let versions = matching_unity_versions(partial_version, available_unity_versions(&path)?);

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
fn run_unity_cmd(
    partial_version: &Option<String>,
    unity_args: &Option<Vec<String>>,
) -> Result<CmdRunner> {
    let (unity_version, directory) = latest_matching_unity(partial_version)?;

    let mut cmd = Command::new(unity_executable_path(&directory));
    cmd.args(unity_args.as_deref().unwrap_or_default());

    Ok(CmdRunner::new(
        cmd,
        None,
        format!("Running Unity {}", unity_version.to_string_lossy()),
    ))
}

/// Returns command that creates an empty project at the given path.
fn new_project_cmd(
    version_pattern: &Option<String>,
    project_path: &Path,
    unity_args: &Option<Vec<String>>,
    no_git: bool,
) -> Result<CmdRunner> {
    // Check if destination already exists.
    if project_path.exists() {
        return Err(anyhow!(
            "Directory already exists: '{}'",
            project_path.canonicalize()?.to_string_lossy()
        ));
    }

    // Create closure that initializes git repository.
    let pre_action: Option<Box<FnCmdAction>> = if !no_git {
        let p = project_path.to_owned();
        Some(Box::new(move || git_init(p)))
    } else {
        None
    };

    let (version, unity_directory) = latest_matching_unity(version_pattern)?;

    let mut cmd = Command::new(unity_executable_path(&unity_directory));
    cmd.arg("-createProject")
        .arg(project_path)
        .args(unity_args.as_deref().unwrap_or_default());

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
fn open_project_cmd(
    project_path: &Path,
    partial_version: &Option<String>,
    unity_args: &Option<Vec<String>>,
) -> Result<CmdRunner> {
    // Make sure the project path exists and is formatted correctly.
    let project_path = validate_path(project_path)?;

    let (version, unity_directory) = if partial_version.is_some() {
        latest_matching_unity(partial_version)?
    } else {
        // Get the Unity version the project uses.
        let version: OsString = unity_project_version(&project_path)?.into();

        // Check if that Unity version is installed.
        let directory = Path::new(&installation_root_path()).join(&version);
        if !directory.exists() {
            return Err(anyhow!(
                "Unity version that the project uses is not installed: {}",
                version.to_string_lossy()
            ));
        }
        (version, directory)
    };

    let unity_path = unity_executable_path(&unity_directory);

    // Build the command to execute.
    let mut cmd = Command::new(unity_path);
    cmd.args(["-projectPath", &project_path.to_string_lossy()])
        .args(unity_args.as_deref().unwrap_or_default());

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

/// Returns the Unity version used for the project.
fn unity_project_version<P: AsRef<Path>>(path: P) -> Result<String> {
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
fn installation_root_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        "/Applications/Unity/Hub/Editor/".into()
    } else if cfg!(target_os = "windows") {
        r"C:\Program Files\Unity\Hub\Editor".into()
    } else {
        unimplemented!()
    }
}

/// Returns the path to the executable.
fn unity_executable_path<P: AsRef<Path>>(path: P) -> PathBuf {
    if cfg!(target_os = "macos") {
        Path::new(path.as_ref()).join("Unity.app/Contents/MacOS/Unity")
    } else if cfg!(target_os = "windows") {
        Path::new(path.as_ref()).join(r"Editor\Unity.exe")
    } else {
        unimplemented!()
    }
}

/// Returns list of available versions that match the partial version or Err if there is no matching version.
fn matching_unity_versions(
    partial_version: &Option<String>,
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
            partial_version.as_deref().unwrap_or("<any>")
        ));
    }

    Ok(versions)
}

/// Returns version and directory of the latest installed version that matches the partial version.
fn latest_matching_unity(partial_version: &Option<String>) -> Result<(OsString, PathBuf)> {
    let path = installation_root_path();

    let version = matching_unity_versions(partial_version, available_unity_versions(&path)?)?
        .last()
        .map(|latest| latest.to_owned())
        .unwrap(); // Guaranteed to have at least one entry.

    let full_path = Path::new(&path).join(&version);
    Ok((version, full_path))
}

/// Returns a natural sorted list of available Unity versions.
fn available_unity_versions<P: AsRef<Path>>(path: P) -> Result<Vec<OsString>> {
    let mut versions: Vec<_> = fs::read_dir(&path)?
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

/// Returns valid and existing path.
fn validate_path<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
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

    if path.has_root() {
        return Ok(path.to_owned());
    }

    let mut path = path.canonicalize()?;

    // Unity borks when passing it paths that start with "\\?\". Strip it off!!
    // Todo: This is a naive way of doing it.
    if cfg!(target_os = "windows") {
        let stripped_path = path
            .to_string_lossy()
            .strip_prefix(r"\\?\")
            .map(|p| Path::new(p).to_owned());

        if let Some(stripped_path) = stripped_path {
            path = stripped_path;
        }
    }
    Ok(path)
}

/// Initializes a new git repository with a default Unity specific .gitignore.
pub fn git_init<P: AsRef<Path>>(path: P) -> Result<()> {
    Command::new("git")
        .arg("init")
        .arg(path.as_ref())
        .output()
        .map_err(|_| anyhow!(
                "Could not create git repository. Make sure git is available or add the --no-git flag."
            ))?;

    let file_path = path.as_ref().join(".gitignore");
    let file_content = include_str!("include/unity-gitignore.txt");
    let mut file = File::create(file_path)?;
    write!(file, "{}", file_content)?;
    Ok(())
}
