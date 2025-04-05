use anyhow::anyhow;
use std::path::Path;
use yansi::Paint;

use crate::commands::install_cmd::install_version;
use crate::commands::{INDENT, MARK_AVAILABLE, MARK_UNAVAILABLE, println_bold};
use crate::unity::release_api::{FetchMode, SortedReleases};
use crate::unity::{
    ProjectPath, ProjectSettings, ReleaseUpdates, find_available_updates, release_notes_url,
};
use crate::utils::content_cache;
use crate::utils::content_cache::RemoteChangeCheck;
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::StatusLine;

pub fn find_project_updates(
    project_dir: &Path,
    install_latest: bool,
    create_report: bool,
    mode: FetchMode,
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

    print_project_header(&project, create_report);
    println!();

    print_project_version(&updates, create_report)?;

    if create_report {
        download_and_print_release_notes(&updates)?;
    } else if !updates.newer_releases.is_empty() {
        println!();
        print_available_updates(&updates.newer_releases)?;
    }

    if create_report {
        Ok(())
    } else {
        handle_newer_release_installation(install_latest, &updates.newer_releases)
    }
}

fn download_and_print_release_notes(updates: &ReleaseUpdates) -> anyhow::Result<()> {
    let status = StatusLine::new("Downloading", "Unity release notes...");
    for release in updates.newer_releases.iter() {
        status.update_line(
            "Downloading",
            &format!("Unity {} release notes...", release.version),
        );

        let url = &release.release_notes.url;
        let body = content_cache::fetch_content(url, RemoteChangeCheck::Validate)?;

        println!();
        println!("## Release notes for [{}]({url})", release.version);
        println!();
        println!("[install in Unity HUB]({})", release.unity_hub_deep_link);

        println!();
        println!("{body}");
    }
    Ok(())
}

fn handle_newer_release_installation(
    install_latest: bool,
    releases: &SortedReleases,
) -> anyhow::Result<()> {
    if let Some(newer_release) = releases.iter().last() {
        let is_installed = newer_release.version.is_editor_installed()?;
        match (is_installed, install_latest) {
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
                    "--install-latest".bold(),
                    newer_release.version.bold()
                );
            }
            _ => { /* The latest version is already installed. */ }
        }
    }
    Ok(())
}

fn print_project_header(project: &ProjectPath, create_report: bool) {
    if create_report {
        print!("# ");
    }

    println_bold!("Unity updates for: `{}`", project.normalized_display());

    if create_report {
        println!();
    }

    match ProjectSettings::from_project(project) {
        Ok(ps) => {
            println!("{}Product name:  {}", INDENT, ps.product_name.bold());
            println!("{}Company name:  {}", INDENT, ps.company_name.bold());
            println!("{}Version:       {}", INDENT, ps.bundle_version.bold());
        }

        Err(e) => {
            println!(
                "{INDENT}{}: {}",
                "Could not read project settings".yellow(),
                e.yellow()
            );
        }
    }
}

fn print_project_version(updates: &ReleaseUpdates, create_report: bool) -> anyhow::Result<()> {
    let is_installed = updates.current_release.version.is_editor_installed()?;
    print!("{}", "Unity editor status: ".bold());

    let version = match (is_installed, updates.newer_releases.is_empty()) {
        (true, true) => {
            println!("{}", "installed (latest version)".green().bold());
            updates.current_release.version.green()
        }
        (true, false) => {
            println!("{}", "installed (update available)".yellow().bold());
            updates.current_release.version.yellow()
        }
        (false, true) => {
            println!("{}", "not installed (latest version)".red().bold());
            updates.current_release.version.red()
        }
        (false, false) => {
            println!("{}", "not installed (outdated version)".red().bold());
            updates.current_release.version.red()
        }
    };

    if create_report {
        println!();
    }

    let installed_marker = if is_installed {
        MARK_AVAILABLE.green()
    } else {
        MARK_UNAVAILABLE.red()
    };

    print!(
        "{}{}{} ({}) - {}",
        installed_marker,
        " ".repeat(INDENT.len() - 1),
        version,
        updates.current_release.release_date.format("%Y-%m-%d"),
        release_notes_url(updates.current_release.version).bright_blue(),
    );

    if is_installed {
        // The editor used by the project is installed, finish the line.
        println!();
    } else if create_report {
        // The editor used by the project is not installed, and we're writing to a file.
        println!(
            " > [install in Unity HUB]({})",
            updates.current_release.unity_hub_deep_link
        );
    } else {
        // The editor used by the project is not installed, and we're writing to the terminal.
        println!();
    }

    Ok(())
}

fn print_available_updates(releases: &SortedReleases) -> anyhow::Result<()> {
    println_bold!("Available update(s):");
    let max_len = releases
        .iter()
        .map(|rd| rd.version.to_interned_str().len())
        .max()
        .ok_or_else(|| anyhow!("No releases"))?;

    for release in releases.iter() {
        let release_date = release.release_date.format("%Y-%m-%d");

        print!(
            "- {:<max_len$} ({}) - {}",
            release.version.to_interned_str().blue().bold(),
            release_date,
            release_notes_url(release.version).bright_blue(),
        );

        if release.version.is_editor_installed()? {
            println!(" > {}", "installed".bold());
        } else {
            println!();
        }
    }

    Ok(())
}
