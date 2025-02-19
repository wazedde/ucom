use anyhow::anyhow;
use std::io::Write;
use std::path::Path;
use yansi::Paint;

use crate::commands::install_cmd::install_version;
use crate::commands::status_line::StatusLine;
use crate::commands::{writeln_b, INDENT};
use crate::unity::release_api::{Mode, SortedReleaseCollection};
use crate::unity::release_api_data::ReleaseData;
use crate::unity::*;

/// Checks on the Unity website for updates to the version used by the project.
pub(crate) fn find_updates(
    project_dir: &Path,
    install_update: bool,
    create_report: bool,
    mode: Mode,
) -> anyhow::Result<()> {
    let project = ProjectPath::try_from(project_dir)?;
    let current_version = project.unity_version()?;

    let updates = {
        let _status = StatusLine::new("Checking", &format!("for updates to {current_version}"));
        find_available_updates(current_version, mode)?
    };

    if create_report {
        yansi::disable();
    }

    let mut buf = Vec::new();
    write_project_header(&project, create_report, &mut buf)?;
    writeln!(buf)?;

    write_project_version(&updates, create_report, &mut buf)?;

    if create_report {
        let download_status = StatusLine::new("Downloading", "Unity release notes...");
        for release in updates.newer_releases.iter() {
            download_status.update(
                "Downloading",
                &format!("Unity {} release notes...", release.version),
            );

            write_release_notes(&mut buf, release)?;
        }
        drop(download_status);
        print!("{}", String::from_utf8(buf)?);
    } else {
        if !updates.newer_releases.is_empty() {
            writeln!(buf)?;
            write_available_updates(&updates.newer_releases, &mut buf)?;
        }
        print!("{}", String::from_utf8(buf)?);
    }

    handle_newer_release_installation(install_update, &updates.newer_releases)
}

fn handle_newer_release_installation(
    install_update: bool,
    releases: &SortedReleaseCollection,
) -> anyhow::Result<()> {
    if let Some(newer_release) = releases.iter().last() {
        let is_installed = newer_release.version.is_editor_installed()?;
        match (is_installed, install_update) {
            (false, true) => {
                // There is a newer version available, and the user wants to install it.
                println!();
                install_version(newer_release)?;
            }
            (false, false) => {
                // There is a newer version available, but the user has not requested installation.
                println!();
                println!(
                    "Use the `{}` flag to install Unity version {}",
                    "--install".bold(),
                    newer_release.version.bold()
                );
            }
            _ => { /* The latest version is already installed. */ }
        }
    }
    Ok(())
}

fn write_project_header(
    project: &ProjectPath,
    create_report: bool,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    if create_report {
        write!(buf, "# ")?;
    }

    writeln_b!(buf, "Unity updates for: {}", project.as_path().display())?;

    if create_report {
        writeln!(buf)?;
    }

    match ProjectSettings::from_project(project) {
        Ok(ps) => {
            writeln!(buf, "{}Product name:  {}", INDENT, ps.product_name.bold())?;
            writeln!(buf, "{}Company name:  {}", INDENT, ps.company_name.bold())?;
            writeln!(buf, "{}Version:       {}", INDENT, ps.bundle_version.bold())?;
        }

        Err(e) => {
            writeln!(
                buf,
                "{INDENT}{}: {}",
                "Could not read project settings".yellow(),
                e.yellow()
            )?;
        }
    }
    Ok(())
}

fn write_project_version(
    updates: &ReleaseUpdates,
    create_report: bool,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let is_installed = updates.current_release.version.is_editor_installed()?;
    write!(buf, "{}", "Unity editor: ".bold())?;

    let version = match (is_installed, updates.newer_releases.is_empty()) {
        (true, true) => {
            writeln!(buf, "{}", "installed, up to date".green().bold())?;
            updates.current_release.version.green()
        }
        (true, false) => {
            writeln!(
                buf,
                "{}",
                "installed, newer version available".yellow().bold()
            )?;
            updates.current_release.version.yellow()
        }
        (false, true) => {
            writeln!(buf, "{}", "not installed, up to date".red().bold())?;
            updates.current_release.version.red()
        }
        (false, false) => {
            writeln!(
                buf,
                "{}",
                "not installed, newer version available".red().bold()
            )?;
            updates.current_release.version.red()
        }
    };

    if create_report {
        writeln!(buf)?;
    }

    write!(
        buf,
        "{}{} - {}",
        INDENT,
        version,
        release_notes_url(updates.current_release.version).bright_blue()
    )?;

    if is_installed {
        // The editor used by the project is installed, finish the line.
        writeln!(buf)?;
    } else if create_report {
        // The editor used by the project is not installed, and we're writing to a file.
        writeln!(
            buf,
            " > [install in Unity HUB]({})",
            updates.current_release.unity_hub_deep_link
        )?;
    } else {
        // The editor used by the project is not installed, and we're writing to the terminal.
        writeln!(buf)?;
    }

    Ok(())
}

fn write_available_updates(
    releases: &SortedReleaseCollection,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    writeln_b!(buf, "Available update(s):")?;
    let max_len = releases
        .iter()
        .map(|rd| rd.version.string_length())
        .max()
        .ok_or(anyhow!("No releases"))?;

    for release in releases.iter() {
        let release_date = release.release_date.format("%Y-%m-%d");

        write!(
            buf,
            "- {:<max_len$} ({}) - {}",
            release.version.to_string().blue().bold(),
            release_date,
            release_notes_url(release.version).bright_blue(),
        )?;

        if release.version.is_editor_installed()? {
            writeln!(buf, " > {}", "installed".bold())?;
        } else {
            writeln!(buf)?;
        };
    }

    Ok(())
}

fn write_release_notes(buf: &mut Vec<u8>, release: &ReleaseData) -> anyhow::Result<()> {
    let url = &release.release_notes.url;
    let body = content_cache::get_cached_content(url, true)?;

    writeln!(buf)?;
    writeln!(buf, "## Release notes for [{}]({url})", release.version)?;
    writeln!(buf)?;
    writeln!(
        buf,
        "[install in Unity HUB]({})",
        release.unity_hub_deep_link
    )?;

    writeln!(buf)?;
    writeln!(buf, "{}", body)?;

    Ok(())
}
