use anyhow::anyhow;
use std::path::Path;
use yansi::Paint;

use crate::commands::install_cmd::install_version;
use crate::commands::{MARK_AVAILABLE, MARK_UNAVAILABLE};
use crate::unity::release_api::{FetchMode, SortedReleases};
use crate::unity::{
    ProjectPath, ProjectSettings, ReleaseUpdates, find_available_updates, release_notes_url,
};
use crate::utils::content_cache;
use crate::utils::content_cache::RemoteChangeCheck;
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::report::{HeaderLevel, Report};
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
        let _status = StatusLine::new("Checking", format!("for updates to {current_version}"));
        find_available_updates(current_version, mode)?
    };

    let report = if create_report {
        yansi::disable();
        Report::Markdown
    } else {
        Report::Terminal
    };

    print_project_header(&project, &report);
    report.blank_line();

    print_project_version(&updates, &report, create_report)?;

    if create_report {
        download_and_print_release_notes(&updates, &report)?;
    } else if !updates.newer_releases.is_empty() {
        report.blank_line();
        print_available_updates(&updates.newer_releases, &report)?;
    }

    if create_report {
        Ok(())
    } else {
        handle_newer_release_installation(install_latest, &updates.newer_releases, &report)
    }
}

fn download_and_print_release_notes(
    updates: &ReleaseUpdates,
    report: &Report,
) -> anyhow::Result<()> {
    let status = StatusLine::new("Downloading", "Unity release notes...");

    for release in updates.newer_releases.iter() {
        // TODO: Printing updates messes up output when output is terminal.
        status.update_line(
            "Downloading",
            format!("Unity {} release notes...", release.version),
        );

        let url = &release.release_notes.url;
        let body = content_cache::fetch_content(url, RemoteChangeCheck::Validate)?;

        report.blank_line();
        report.header(
            format!("Release notes for [{}]({url})", release.version),
            HeaderLevel::H2,
        );
        report.paragraph(format!(
            "[install in Unity HUB]({})",
            release.unity_hub_deep_link
        ));
        report.blank_line();
        report.paragraph(body);
    }
    Ok(())
}

fn handle_newer_release_installation(
    install_latest: bool,
    releases: &SortedReleases,
    report: &Report,
) -> anyhow::Result<()> {
    if let Some(newer_release) = releases.iter().last() {
        let is_installed = newer_release.version.is_editor_installed()?;
        match (is_installed, install_latest) {
            (false, true) => {
                // There is a newer version available, and the user wants to install it.
                report.blank_line();
                install_version(newer_release)?;
            }
            (false, false) => {
                // There is a newer version available, but the user has not requested installation.
                report.blank_line();
                report.paragraph(format!(
                    "Use the `{}` flag to install Unity version {}",
                    "--install-latest".bold(),
                    newer_release.version.bold()
                ));
            }
            _ => { /* The latest version is already installed. */ }
        }
    }
    Ok(())
}

fn print_project_header(project: &ProjectPath, report: &Report) {
    report.header(
        format!("Unity updates for: `{}`", project.normalized_display()),
        HeaderLevel::H1,
    );

    match ProjectSettings::from_project(project) {
        Ok(ps) => {
            report.list_item(format!("Product name:  {}", ps.product_name.bold()));
            report.list_item(format!("Company name:  {}", ps.company_name.bold()));
            report.list_item(format!("Version:       {}", ps.bundle_version.bold()));
        }

        Err(e) => {
            report.list_item(format!(
                "{}: {}",
                "Could not read project settings".yellow(),
                e.yellow()
            ));
        }
    }
}

fn print_project_version(
    updates: &ReleaseUpdates,
    report: &Report,
    create_report: bool,
) -> anyhow::Result<()> {
    let is_installed = updates.current_release.version.is_editor_installed()?;

    let (status, colored_version) = match (is_installed, updates.newer_releases.is_empty()) {
        (true, true) => (
            "installed (latest version)".green(),
            updates.current_release.version.green(),
        ),
        (true, false) => (
            "installed (update available)".yellow(),
            updates.current_release.version.yellow(),
        ),
        (false, true) => (
            "not installed (latest version)".red(),
            updates.current_release.version.red(),
        ),
        (false, false) => (
            "not installed (outdated version)".red(),
            updates.current_release.version.red(),
        ),
    };

    report.header(format!("Unity editor status: {status}"), HeaderLevel::H2);

    let installed_marker = if is_installed {
        MARK_AVAILABLE.green()
    } else {
        MARK_UNAVAILABLE.red()
    };

    report.marked_item(
        format!(
            "{} ({}) - {}{}",
            colored_version,
            updates.current_release.release_date.format("%Y-%m-%d"),
            release_notes_url(updates.current_release.version).bright_blue(),
            if is_installed {
                // The editor used by the project is installed, finish the line.
                String::default()
            } else if create_report {
                // The editor used by the project is not installed, and we're writing to a file.
                format!(
                    " > [install in Unity HUB]({})",
                    updates.current_release.unity_hub_deep_link
                )
            } else {
                // The editor used by the project is not installed, and we're writing to the terminal.
                String::default()
            }
        ),
        installed_marker,
    );

    Ok(())
}

fn print_available_updates(releases: &SortedReleases, report: &Report) -> anyhow::Result<()> {
    report.header("Available update(s):", HeaderLevel::H2);
    let max_len = releases
        .iter()
        .map(|rd| rd.version.to_interned_str().len())
        .max()
        .ok_or_else(|| anyhow!("No releases"))?;

    for release in releases.iter() {
        report.marked_item(
            format!(
                "{:<max_len$} ({}) - {}{}",
                release.version.to_interned_str().blue().bold(),
                release.release_date.format("%Y-%m-%d"),
                release_notes_url(release.version).bright_blue(),
                if release.version.is_editor_installed()? {
                    format!(" > {}", "installed".bold())
                } else {
                    String::default()
                }
            ),
            '-',
        );
    }

    Ok(())
}
