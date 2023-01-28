use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Command;
use std::{env, fs};

use anyhow::{anyhow, Error, Result};
use colored::{ColoredString, Colorize};
use indexmap::IndexSet;
use path_absolutize::Absolutize;
use spinoff::{Color, Spinner, Spinners};

use crate::build_script;
use crate::cli::*;
use crate::unity_cmd::*;
use crate::unity_project::*;
use crate::unity_release::*;
use crate::unity_version::UnityVersion;

const GIT_IGNORE: &str = include_str!("include/unity-gitignore.txt");

/// Lists installed Unity versions.
pub fn list_versions(list_type: ListType, partial_version: Option<&str>) -> Result<()> {
    let dir = editor_parent_dir()?;
    let matching_versions = matching_versions(available_unity_versions(&dir)?, partial_version)?;

    match list_type {
        ListType::Installed => {
            println!(
                "{}",
                format!("Unity versions in `{}`", dir.display()).bold()
            );

            print_installed_versions(&matching_versions, &Vec::new())?;
        }
        ListType::Updates => {
            println!(
                "{}",
                format!("Updates for Unity versions in `{}`", dir.display()).bold()
            );
            let spinner = Spinner::new(Spinners::Dots, "Downloading release data...", Color::White);
            let releases = request_unity_releases()?;
            spinner.clear();
            print_installed_versions(&matching_versions, &releases)?;
        }
        ListType::Latest => {
            println!("{}", "Latest available point releases".bold());
            let spinner = Spinner::new(Spinners::Dots, "Downloading release data...", Color::White);
            let releases = request_unity_releases()?;
            spinner.clear();
            print_latest_versions(&matching_versions, &releases, partial_version);
        }
    }

    Ok(())
}

fn print_installed_versions(installed: &[UnityVersion], available: &[ReleaseInfo]) -> Result<()> {
    let default_version = env::var_os(ENV_DEFAULT_VERSION)
        .and_then(|env| {
            installed
                .iter()
                .rev()
                .find(|v| v.to_string().starts_with(env.to_string_lossy().as_ref()))
        })
        .or_else(|| installed.last())
        .copied()
        .ok_or_else(|| anyhow!("No Unity versions installed"))?;

    let installed: Vec<_> = installed.iter().map(|v| (v, v.to_string())).collect();
    let max_len = installed.iter().map(|(_, s)| s.len()).max().unwrap();

    let mut previous_range = None;
    let mut iter = installed.iter().peekable();

    while let Some((&version, version_string)) = iter.next() {
        let is_next_in_same_range = iter.peek().map_or(false, |(v, _)| {
            v.year == version.year && v.point == version.point
        });

        if Some((version.year, version.point)) == previous_range {
            if is_next_in_same_range {
                print!("├─ ");
            } else {
                print!("└─ ");
            }
        } else {
            previous_range = Some((version.year, version.point));
            if is_next_in_same_range {
                print!("┬─ ");
            } else {
                print!("── ");
            }
        }

        let mut colorize_line: fn(&str) -> ColoredString = |s: &str| s.into();
        let mut line = format!("{version_string:<max_len$}");

        if !available.is_empty() && !is_next_in_same_range {
            let range: Vec<_> = available
                .iter()
                .map(|r| r.version)
                .filter(|v| v.year == version.year && v.point == version.point)
                .collect();

            if let Some(latest) = range.last() {
                match version.cmp(latest) {
                    Ordering::Equal => {
                        // Latest version in the range.
                        line.push_str(" - Up to date");
                    }
                    Ordering::Less => {
                        // Later version available.
                        colorize_line = |s: &str| s.yellow().bold();
                        line.push_str(&format!(
                            " - Update available: {} behind {}",
                            range.iter().filter(|&&v| v > version).count(),
                            latest
                        ));
                    }
                    Ordering::Greater => {
                        // Installed version is newer than latest available.
                        line.push_str(" - Newer than latest available");
                    }
                }
            } else {
                // No releases in the x.y range.
                line.push_str(" - No update information available");
            }
        }

        if version == default_version {
            line.push_str(" (default for new projects)");
            println!("{}", colorize_line(&line).bold());
        } else {
            println!("{}", colorize_line(&line));
        }
    }

    Ok(())
}

fn print_latest_versions(
    installed: &[UnityVersion],
    available: &[ReleaseInfo],
    partial_version: Option<&str>,
) {
    // Get the latest version of each range.
    let latest: Vec<_> = {
        let mut available_ranges: Vec<_> = available
            .iter()
            .filter(|r| partial_version.map_or(true, |p| r.version.to_string().starts_with(p)))
            .map(|r| (r.version.year, r.version.point))
            .collect();

        available_ranges.sort_unstable();
        available_ranges.dedup();

        available_ranges
            .iter()
            .filter_map(|&(year, point)| {
                available
                    .iter()
                    .map(|r| r.version)
                    .filter(|v| v.year == year && v.point == point)
                    .max()
            })
            .map(|v| (v, v.to_string()))
            .collect()
    };

    let max_len = latest.iter().map(|(_, s)| s.len()).max().unwrap_or(0);

    let mut previous_range = None;
    let mut iter = latest.iter().peekable();

    while let Some((latest_version, latest_string)) = iter.next() {
        let is_next_in_same_range = iter
            .peek()
            .map_or(false, |(v, _)| v.year == latest_version.year);
        if Some(latest_version.year) == previous_range {
            if is_next_in_same_range {
                print!("├─ ");
            } else {
                print!("└─ ");
            }
        } else {
            previous_range = Some(latest_version.year);
            if is_next_in_same_range {
                print!("┬─ ");
            } else {
                print!("── ");
            }
        };

        // Find all installed versions in the same range as the latest version.
        let installed_in_range: Vec<_> = installed
            .iter()
            .filter(|v| v.year == latest_version.year && v.point == latest_version.point)
            .copied()
            .collect();

        // Concatenate the versions for printing.
        let joined = installed_in_range
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        if installed_in_range.is_empty() {
            // No installed versions in the range.
            println!("{latest_string}");
        } else if installed_in_range
            .last()
            .filter(|&v| v == latest_version)
            .is_some()
            || installed_in_range // Special case for when installed version is newer than latest.
                .last()
                .map_or(false, |v| v > latest_version)
        {
            // No updates to the latest version are available.
            println!(
                "{}",
                format!("{latest_string:<max_len$} - Installed: {joined}").bold()
            );
        } else {
            // Update to the latest version is available.
            println!(
                "{}",
                format!("{latest_string:<max_len$} - Installed: {joined} *update available")
                    .yellow()
                    .bold()
            );
        }
    }
}

/// Shows project information.
pub fn show_project_info(project_dir: &Path, packages_level: PackagesInfoLevel) -> Result<()> {
    let project_dir = validate_project_path(&project_dir)?;
    let version = version_used_by_project(&project_dir)?;

    println!(
        "{}",
        format!("Project info for `{}`", project_dir.display()).bold()
    );

    if let Ok(settings) = ProjectSettings::from_project(&project_dir) {
        let ps = settings.player_settings;
        println!("    Product Name:  {}", ps.product_name.bold());
        println!("    Company Name:  {}", ps.company_name.bold());
        println!("    Version:       {}", ps.bundle_version.bold());
    }

    print!("    Unity Version: {}", version.to_string().bold());
    if is_editor_installed(version)? {
        println!();
    } else {
        println!(" {}", "*not installed".red().bold());
    }
    if packages_level != PackagesInfoLevel::None {
        show_project_packages(project_dir.as_ref(), packages_level);
    };

    Ok(())
}

/// Checks on the Unity website for updates to the version used by the project.
pub fn check_unity_updates(project_dir: &Path, report_path: Option<&Path>) -> Result<()> {
    // TODO: This function is too long and messy, split it up.
    let project_dir = validate_project_path(&project_dir)?;

    if let Some(path) = report_path {
        validate_report_path(path)?;
    }

    let version = version_used_by_project(&project_dir)?;

    let mut spinner = Spinner::new(
        Spinners::Dots,
        format!("Project uses {version}; checking for updates..."),
        Color::White,
    );

    let mut buf = Vec::new();

    if report_path.is_some() {
        write!(buf, "# ")?;
    }

    let product_name = ProjectSettings::from_project(&project_dir).map_or_else(
        |_| "<UNKNOWN>".to_string(),
        |s| s.player_settings.product_name,
    );

    writeln!(
        buf,
        "{}",
        style_white(
            &format!("Unity updates for {product_name}"),
            report_path.is_none(),
        )
    )?;

    drop(product_name);

    if report_path.is_some() {
        writeln!(buf)?;
    }

    writeln!(
        buf,
        "    Directory:            {}",
        style_white(&project_dir.to_string_lossy(), report_path.is_none())
    )?;

    write!(
        buf,
        "    Project uses version: {}",
        style_white(&version.to_string(), report_path.is_none())
    )?;

    if is_editor_installed(version)? {
        writeln!(buf)?;
    } else {
        writeln!(
            buf,
            " {}",
            style_red("*not installed", report_path.is_none())
        )?;
    }

    let releases = request_updates_for(version)?;
    if releases.is_empty() {
        writeln!(
            buf,
            "    Already uses the latest release in the {}.{}.x range",
            version.year, version.point
        )?;
        spinner.clear();
        print!("{}", String::from_utf8(buf)?);
        return Ok(());
    }

    let latest = releases.last().unwrap();
    writeln!(
        buf,
        "    Update available:     {}",
        style_yellow(&latest.version.to_string(), report_path.is_none())
    )?;

    if report_path.is_none() {
        spinner.clear();
        print!("{}", String::from_utf8(buf)?);
        return Ok(());
    }

    for release in releases {
        spinner.update_text(format!(
            "Downloading Unity {} release notes...",
            release.version
        ));

        fetch_release_notes(&mut buf, &release)?;
    }
    spinner.clear();

    match report_path {
        None => {
            print!("{}", String::from_utf8(buf)?);
        }
        Some(file_name) => {
            fs::write(file_name, String::from_utf8(buf)?)?;
            println!(
                "Update report written to: {}",
                file_name.absolutize()?.display()
            );
        }
    };
    Ok(())
}

fn validate_report_path(path: &Path) -> Result<()> {
    if path.is_dir() {
        return Err(anyhow!(
            "The report file name provided is a directory: {}",
            path.display()
        ));
    }

    if path
        .extension()
        .filter(|e| e == &OsStr::new("md"))
        .is_none()
    {
        return Err(anyhow!(
            "Make sure the report file name has the `md` extension: {}",
            path.display()
        ));
    }

    Ok(())
}

fn style_white(s: &str, apply: bool) -> ColoredString {
    if apply {
        s.bold()
    } else {
        s.into()
    }
}

fn style_red(s: &str, apply: bool) -> ColoredString {
    if apply {
        s.red().bold()
    } else {
        s.into()
    }
}

fn style_yellow(s: &str, apply: bool) -> ColoredString {
    if apply {
        s.yellow().bold()
    } else {
        s.into()
    }
}

fn fetch_release_notes(buf: &mut Vec<u8>, release: &ReleaseInfo) -> Result<()> {
    let url = release_notes_url(release.version);
    let body = ureq::get(&url).call()?.into_string()?;

    let release_notes = collect_release_notes(&body);
    if release_notes.is_empty() {
        return Ok(());
    }

    writeln!(buf)?;
    writeln!(buf, "## Release notes for [{}]({url})", release.version)?;

    for (header, entries) in release_notes {
        writeln!(buf)?;
        writeln!(buf, "### {header}")?;
        writeln!(buf)?;
        for e in &entries {
            writeln!(buf, "- {e}")?;
        }
    }
    Ok(())
}

/// Show packages used by the project.
fn show_project_packages(project_dir: &Path, package_level: PackagesInfoLevel) {
    let Ok(packages) = Packages::from_project(project_dir) else {
        return;
    };

    let packages: Vec<_> = packages
        .dependencies
        .iter()
        .filter(|(_, package)| package_level.eval(package))
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

impl PackagesInfoLevel {
    fn eval(self, package: &PackageInfo) -> bool {
        match self {
            Self::None => false,
            Self::NonUnity => {
                package.depth == 0
                    && (package.source == "git"
                        || package.source == "embedded"
                        || package.source == "local")
            }
            Self::Registry => {
                package.depth == 0
                    && (package.source == "git"
                        || package.source == "embedded"
                        || package.source == "local"
                        || package.source == "registry")
            }
            Self::All => true,
        }
    }
}

/// Runs the Unity Editor with the given arguments.
pub fn run_unity(arguments: RunArguments) -> Result<()> {
    let (version, editor_exe) = matching_editor(arguments.version_pattern.as_deref())?;

    let mut cmd = Command::new(editor_exe);
    cmd.args(arguments.args.unwrap_or_default());

    if arguments.dry_run {
        println!("{}", cmd_to_string(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println!("{}", format!("Run Unity {version}").bold());
    }

    if arguments.wait {
        wait_with_stdout(cmd)
    } else {
        spawn_and_forget(cmd)
    }
}

/// Creates a new Unity project and optional Git repository in the given directory.
pub fn new_project(arguments: NewArguments) -> Result<()> {
    let project_dir = arguments.project_dir.absolutize()?;

    if project_dir.exists() {
        return Err(anyhow!(
            "Directory already exists: `{}`",
            project_dir.absolutize()?.display()
        ));
    }

    let (version, editor_exe) = matching_editor(arguments.version_pattern.as_deref())?;

    let mut cmd = Command::new(editor_exe);
    cmd.arg("-createProject")
        .arg(project_dir.as_ref())
        .args(arguments.args.unwrap_or_default());

    if arguments.quit {
        cmd.arg("-quit");
    }

    if arguments.dry_run {
        println!("{}", cmd_to_string(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println!(
            "{}",
            format!(
                "Create new Unity {} project in `{}`",
                version,
                project_dir.display()
            )
            .bold()
        );
    }

    if !arguments.no_git {
        git_init(project_dir)?;
    }

    if arguments.wait {
        wait_with_stdout(cmd)?;
    } else {
        spawn_and_forget(cmd)?;
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
        println!("{}", cmd_to_string(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println!(
            "{}",
            format!(
                "Open Unity {} project in `{}`",
                version,
                project_dir.display()
            )
            .bold()
        );
    }

    if arguments.wait {
        wait_with_stdout(cmd)?;
    } else {
        spawn_and_forget(cmd)?;
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
            project_dir.display()
        ));
    }

    let Some(log_file) = arguments.log_file.file_name() else {
        return Err(anyhow!("Invalid log file name: `{}`", arguments.log_file.display()));
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
        BuildMode::Editor => (), // Do nothing.
    }

    // Add any additional arguments.
    cmd.args(arguments.args.as_deref().unwrap_or_default());

    if arguments.dry_run {
        println!("{}", cmd_to_string(&cmd));
        return Ok(());
    }

    println!(
        "{}",
        format!(
            "Building Unity {version} {} project in `{}`",
            arguments.target,
            project_dir.display()
        )
        .bold()
    );

    let (inject_build_script, remove_build_script) =
        build_script::new_build_script_injection_functions(&project_dir, arguments.inject);

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

/// Returns errors from the given log file as one collected Err.
fn errors_from_log(log_file: &Path) -> Error {
    let Ok(log_file) = File::open(log_file) else {
        return anyhow!("Failed to open log file: `{}`", log_file.display());
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

/// Initializes a new git repository with a default Unity specific .gitignore.
fn git_init<P: AsRef<Path>>(project_dir: P) -> Result<()> {
    let project_dir = project_dir.as_ref();
    Command::new("git")
        .arg("init")
        .arg(project_dir)
        .output()
        .map_err(|_| anyhow!("Could not create git repository. Make sure git is available or add the --no-git flag."))?;

    let mut file = File::create(project_dir.join(".gitignore"))?;
    write!(file, "{GIT_IGNORE}").map_err(Into::into)
}

/// Debug function to easily filter out releases.
#[allow(dead_code)]
fn remove_from_releases(partial: &str, releases: Vec<ReleaseInfo>) -> Vec<ReleaseInfo> {
    let releases: Vec<_> = releases
        .into_iter()
        .filter(|v| !v.version.to_string().starts_with(partial))
        .collect();
    releases
}
