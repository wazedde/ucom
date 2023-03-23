use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::anyhow;
use colored::Colorize;
use path_absolutize::Absolutize;
use spinoff::{spinners, Spinner};

use crate::unity::*;

/// Checks on the Unity website for updates to the version used by the project.
pub fn check_updates(project_dir: &Path, report_path: Option<&Path>) -> anyhow::Result<()> {
    let project_dir = validate_project_path(&project_dir)?;
    let output_to_file = report_path.is_some();

    if let Some(path) = report_path {
        validate_report_path(path)?;
    }

    let project_version = version_used_by_project(&project_dir)?;

    let spinner = Spinner::new(
        spinners::Dots,
        format!("Project uses {}; checking for updates...", project_version),
        None,
    );

    let (project_version_info, updates) = request_patch_update_info(project_version)?;
    spinner.clear();

    if output_to_file {
        // Disable colored output when writing to a file.
        colored::control::set_override(false);
    }

    let mut buf = Vec::new();

    write_project_header(&project_dir, output_to_file, &mut buf)?;

    writeln!(buf)?;

    write_project_version(
        project_version,
        project_version_info,
        &updates,
        output_to_file,
        &mut buf,
    )?;

    if output_to_file {
        let mut spinner = Spinner::new(spinners::Dots, "Downloading Unity release notes...", None);
        for release in updates {
            spinner.update_text(format!(
                "Downloading Unity {} release notes...",
                release.version
            ));

            write_release_notes(&mut buf, &release)?;
        }
        spinner.clear();

        let file_name = report_path.expect("Already validated");
        fs::write(file_name, String::from_utf8(buf)?)?;
        println!(
            "Update report written to: {}",
            file_name.absolutize()?.display()
        );
    } else {
        if !updates.is_empty() {
            writeln!(buf)?;
            write_available_updates(&updates, &mut buf)?;
        }
        print!("{}", String::from_utf8(buf)?);
    }

    Ok(())
}

fn write_project_header(
    project_dir: &Path,
    output_to_file: bool,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    if output_to_file {
        write!(buf, "# ")?;
    }

    let product_name = ProjectSettings::from_project(project_dir).map_or_else(
        |_| "<UNKNOWN>".to_string(),
        |s| s.player_settings.product_name,
    );

    let s = format!("Unity updates for project: {product_name}").bold();
    writeln!(buf, "{s}")?;

    if output_to_file {
        writeln!(buf)?;
    }

    writeln!(buf, "- Directory: {}", project_dir.to_string_lossy().bold())?;
    Ok(())
}

fn write_project_version(
    project_version: UnityVersion,
    project_version_info: Option<ReleaseInfo>,
    updates: &[ReleaseInfo],
    output_to_file: bool,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let is_installed = is_editor_installed(project_version)?;
    write!(buf, "{}", "The version the project uses is ".bold())?;

    if is_installed {
        if updates.is_empty() {
            writeln!(buf, "{}", "installed and up to date:".bold())?;
        } else {
            writeln!(buf, "{}", "installed:".bold())?;
        }
    } else {
        writeln!(buf, "{}", "not installed:".red().bold())?;
    }

    if output_to_file {
        writeln!(buf)?;
    }

    write!(
        buf,
        "- {} - {}",
        project_version,
        release_notes_url(project_version)
    )?;

    if is_installed {
        // The editor used by the project is installed, finish the line.
        writeln!(buf)?;
    } else if output_to_file {
        // The editor used by the project is not installed, and we're writing to a file.
        writeln!(
            buf,
            " > {}",
            project_version_info
                .map(|r| r.installation_url)
                .map_or_else(
                    || "No release info available".to_string(),
                    |s| format!("[install in Unity HUB]({})", s)
                )
                .bold()
        )?;
    } else {
        // The editor used by the project is not installed, and we're not writing to a file.
        writeln!(
            buf,
            " > {}",
            project_version_info
                .map_or_else(
                    || "No release info available".to_string(),
                    |r| r.installation_url
                )
                .bold()
        )?;
    }

    Ok(())
}

fn write_available_updates(updates: &[ReleaseInfo], buf: &mut Vec<u8>) -> anyhow::Result<()> {
    writeln!(buf, "{}", "Update(s) available:".bold())?;

    let max_len = updates.iter().map(|ri| ri.version.len()).max().unwrap();

    for release in updates {
        if is_editor_installed(release.version).unwrap_or(false) {
            // The editor is installed, but not used by the project.
            writeln!(
                buf,
                "- {:<max_len$} - {} > {}",
                release.version.to_string().yellow().bold(),
                release_notes_url(release.version),
                "installed".bold()
            )?;
        } else {
            // The editor is not installed.
            writeln!(
                buf,
                "- {:<max_len$} - {} > {}",
                release.version.to_string().yellow().bold(),
                release_notes_url(release.version),
                release.installation_url.bold(),
            )?;
        }
    }

    Ok(())
}

fn validate_report_path(path: &Path) -> anyhow::Result<()> {
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

fn write_release_notes(buf: &mut Vec<u8>, release: &ReleaseInfo) -> anyhow::Result<()> {
    let (url, body) = request_release_notes(release.version)?;
    let release_notes = extract_release_notes(&body);
    if release_notes.is_empty() {
        return Ok(());
    }

    writeln!(buf)?;
    writeln!(buf, "## Release notes for [{}]({url})", release.version)?;
    writeln!(buf)?;
    writeln!(buf, "[install in Unity HUB]({})", release.installation_url)?;

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
