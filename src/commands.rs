use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Command;
use std::{env, fs};

use anyhow::{anyhow, Error, Result};
use colored::{ColoredString, Colorize};
use indexmap::IndexSet;
use path_absolutize::Absolutize;
use tokio::runtime::Runtime;

use crate::build_script;
use crate::cli::*;
use crate::command_ext::*;
use crate::unity_project::*;
use crate::unity_release::fetch_releases;

const GIT_IGNORE: &str = include_str!("include/unity-gitignore.txt");

/// Lists installed Unity versions.
pub fn list_versions(partial_version: Option<&str>, check_updates: bool) -> Result<()> {
    let dir = editor_parent_dir()?;
    let versions = available_unity_versions(&dir)?;

    let default_version = env::var_os(ENV_DEFAULT_VERSION)
        .and_then(|env_version| {
            versions.iter().rev().find(|v| {
                v.to_string()
                    .starts_with(env_version.to_string_lossy().as_ref())
            })
        })
        .unwrap_or_else(|| versions.last().unwrap())
        .to_owned();

    let versions = matching_versions(versions, partial_version)?;

    println!(
        "{}",
        format!("Unity versions in `{}`", dir.to_string_lossy()).bold()
    );

    let releases = if check_updates {
        Runtime::new()?.block_on(fetch_releases())?
    } else {
        Vec::new()
    };

    let versions: Vec<_> = versions.iter().map(|v| (v, v.to_string())).collect();
    let max_width = versions.iter().map(|(_, s)| s.len()).max().unwrap_or(0);

    let plain_color: fn(&str) -> ColoredString = |s: &str| s.into();
    let mut line = String::new();
    let mut colorize_line;

    for (version, version_string) in versions {
        colorize_line = plain_color;
        line.push_str(&format!("{:<max_width$}", version_string));

        if check_updates {
            let r: Vec<_> = releases
                .iter()
                .filter(|r| {
                    r.version.year == version.year
                        && r.version.point == version.point
                        && r.version > *version
                })
                .collect();

            if r.is_empty() {
                line.push_str(" - latest");
            } else {
                colorize_line = |s: &str| s.yellow().bold();
                line.push_str(&format!(
                    " - {} behind {}",
                    r.len(),
                    r.last().unwrap().version
                ));
            }
        }

        if *version == default_version {
            line.push_str(" (default for new projects)");
            println!("{}", colorize_line(line.trim()).bold());
        } else {
            println!("{}", colorize_line(line.trim()));
        }

        line.clear();
    }
    Ok(())
}

/// Shows project information.
pub fn show_project_info(project_dir: &Path, packages_level: PackagesInfoLevel) -> Result<()> {
    let project_dir = validate_project_path(&project_dir)?;
    let version = version_used_by_project(&project_dir)?;

    println!(
        "{}",
        format!("Project info for `{}`", project_dir.to_string_lossy()).bold()
    );

    if let Ok(settings) = ProjectSettings::from_project(&project_dir) {
        let ps = settings.player_settings;
        println!("    Product Name:  {}", ps.product_name.bold(),);
        println!("    Company Name:  {}", ps.company_name.bold(),);
        println!("    Version:       {}", ps.bundle_version.bold());
    }

    print!("    Unity Version: {}", version.to_string().bold());
    if editor_parent_dir()?.join(version.to_string()).exists() {
        println!();
    } else {
        println!(" {}", "*not installed".red().bold());
    }
    if packages_level != PackagesInfoLevel::None {
        show_project_packages(project_dir.as_ref(), packages_level);
    };

    Ok(())
}

/// Show packages used by the project.
pub fn show_project_packages(project_dir: &Path, level: PackagesInfoLevel) {
    let Ok(packages) = Packages::from_project(project_dir) else {
        return;
    };

    let packages: Vec<_> = packages
        .dependencies
        .iter()
        .filter(|(_, package)| match level {
            PackagesInfoLevel::None => false,
            PackagesInfoLevel::Some => {
                package.depth == 0
                    && (package.source == "git"
                        || package.source == "embedded"
                        || package.source == "local")
            }
            PackagesInfoLevel::More => {
                package.depth == 0
                    && (package.source == "git"
                        || package.source == "embedded"
                        || package.source == "local"
                        || package.source == "registry")
            }
            PackagesInfoLevel::Most => true,
        })
        .collect();

    if packages.is_empty() {
        return;
    }

    println!(
        "{}",
        "Packages (L=local, E=embedded, G=git, R=registry, B=builtin)".bold()
    );
    for (name, package) in packages {
        println!(
            "    {} {} ({})",
            package.source.chars().next().unwrap_or(' ').to_uppercase(),
            name,
            package.version
        );
    }
}

/// Runs the Unity Editor with the given arguments.
pub fn run_unity(arguments: RunArguments) -> Result<()> {
    let (version, editor_exe) = matching_editor(arguments.version_pattern.as_deref())?;

    let mut cmd = Command::new(editor_exe);
    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", cmd.to_command_line_string());
        return Ok(());
    }

    if !arguments.quiet {
        println!("{}", format!("Run Unity {}", version.to_string()).bold());
    }

    if arguments.wait {
        cmd.wait_with_stdout()
    } else {
        cmd.forget()
    }
}

/// Creates a new Unity project and optional Git repository in the given directory.
pub fn new_project(arguments: NewArguments) -> Result<()> {
    let project_dir = arguments.project_dir.absolutize()?;

    if project_dir.exists() {
        return Err(anyhow!(
            "Directory already exists: `{}`",
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
            "{}",
            format!(
                "Create new Unity {} project in `{}`",
                version,
                project_dir.to_string_lossy()
            )
            .bold()
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
pub fn open_project(arguments: OpenArguments) -> Result<()> {
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
            "{}",
            format!(
                "Open Unity {} project in `{}`",
                version,
                project_dir.to_string_lossy()
            )
            .bold()
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
pub fn build_project(arguments: BuildArguments) -> Result<()> {
    let project_dir = validate_project_path(&arguments.project_dir)?;
    let (version, editor_exe) = matching_editor_used_by_project(&project_dir)?;

    let output_dir = arguments.build_path.unwrap_or_else(|| {
        // If no build path is given, use <project>/Builds/<target>
        project_dir
            .join("Builds")
            .join(arguments.target.to_string())
    });

    if project_dir == output_dir {
        return Err(anyhow!(
            "Output directory cannot be the same as the project directory: `{}`",
            project_dir.to_string_lossy()
        ));
    }

    let Some(log_file) = arguments.log_file.file_name() else {
        return Err(anyhow!("Invalid log file name: `{}`", arguments.log_file.to_string_lossy()));
    };

    let log_file = if log_file == arguments.log_file {
        // Log filename without path was given, use the output path as destination.
        output_dir.join(log_file)
    } else {
        log_file.into()
    };

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
        BuildMode::Editor => {} // Do nothing.
    }

    // Add any additional arguments.
    cmd.args(arguments.args.as_deref().unwrap_or_default());

    if arguments.dry_run {
        println!("{}", cmd.to_command_line_string());
        return Ok(());
    }

    println!(
        "{}",
        format!(
            "Building Unity {} {} project in `{}`",
            version,
            arguments.target,
            project_dir.to_string_lossy()
        )
        .bold()
    );

    let (inject_build_script, remove_build_script) =
        build_script::new_build_script_injection_functions(&project_dir, arguments.inject);

    inject_build_script()?;

    let show_log = !arguments.quiet
        && (arguments.mode == BuildMode::Batch || arguments.mode == BuildMode::BatchNoGraphics);

    let build_result = if show_log {
        cmd.wait_with_log_echo(&log_file)
    } else {
        cmd.wait_with_stdout()
    };

    remove_build_script()?;

    if build_result.is_ok() {
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
            .for_each(|l| println!("{}", l));
    }

    build_result.map_err(|_| errors_from_log(&log_file))
}

/// Returns errors from the given log file as one collected Err.
fn errors_from_log(log_file: &Path) -> Error {
    let Ok(log_file) = File::open(log_file) else {
        return anyhow!("Failed to open log file: `{}`", log_file.to_string_lossy());
    };

    let errors: IndexSet<_> = BufReader::new(log_file)
        .lines()
        .flatten()
        .filter(|l| {
            l.starts_with("[Builder] Error:")
                || l.contains("error CS")
                || l.starts_with("Fatal Error")
                || l.starts_with("Error building Player")
                || l.starts_with("error:")
                || l.starts_with("BuildFailedException:")
        })
        .collect();

    match errors.len() {
        0 => anyhow!("No errors found in log"),
        1 => anyhow!("{}", errors[0]),
        _ => {
            let mut joined = String::new();
            for (i, error) in errors.iter().enumerate() {
                joined.push_str(format!("{}: {}\n", format!("{}", i + 1).bold(), error).as_str());
            }
            anyhow!(joined)
        }
    }
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
    write!(file, "{}", GIT_IGNORE).map_err(Into::into)
}
