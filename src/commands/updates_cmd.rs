use anyhow::anyhow;
use yansi::Paint;

use crate::cli_updates::UpdatesArguments;
use crate::commands::install_cmd::install_version;
use crate::commands::output::Mark;
use crate::commands::project_setup::ProjectSetup;
use crate::commands::release_issue::report_error_description;
use crate::style_definitions::{Styled, Tone};
use crate::unity::ProjectPath;
use crate::unity::release_api::{SortedReleases, UpdatePolicy};
use crate::unity::{ProjectSettings, ReleaseUpdates, find_available_updates, release_notes_url};
use crate::utils::content_cache;
use crate::utils::content_cache::RemoteChangeCheck;
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::report::{HeaderLevel, Report};
use crate::utils::status_line::StatusLine;

pub fn find_project_updates(
    arguments: &UpdatesArguments,
    mode: UpdatePolicy,
) -> anyhow::Result<()> {
    let setup = ProjectSetup::new(&arguments.project_dir)?;
    let current_version = setup.unity_version;

    let updates = {
        let _status = StatusLine::new("Checking", format!("for updates to {current_version}"));
        find_available_updates(current_version, mode)?
    };

    let report = if arguments.report {
        yansi::disable();
        Report::Markdown
    } else {
        Report::Terminal
    };

    print_project_header(&setup.project, &report);
    report.blank_line();

    print_project_version(&updates, &report, arguments.report)?;

    if arguments.report {
        download_and_print_release_notes(&updates, &report)?;
    } else if !updates.newer_releases.is_empty() {
        report.blank_line();
        print_available_updates(&updates, &report)?;
    }

    if arguments.report {
        Ok(())
    } else {
        handle_newer_release_installation(
            arguments.install_latest,
            &updates.newer_releases,
            &report,
        )
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
                "Could not read project settings".tone(Tone::Warning),
                e.tone(Tone::Warning),
            ));
        }
    }
}

fn print_project_version(
    updates: &ReleaseUpdates,
    report: &Report,
    create_report: bool,
) -> anyhow::Result<()> {
    let release = &updates.current_release;
    let is_installed = release.version.is_editor_installed()?;

    let issue = release.issue();

    let (status, tone) = match (is_installed, updates.newer_releases.is_empty()) {
        (true, true) => ("installed (latest version)", Tone::Success),
        (true, false) => ("installed (update available)", Tone::Outdated),
        (false, true) => ("not installed (latest version)", Tone::Error),
        (false, false) => ("not installed (outdated version)", Tone::Error),
    };
    let version_label = updates.current_release.version.tone(tone);

    report.header(
        format_args!("Unity editor status: {}", status.tone(tone)),
        HeaderLevel::H2,
    );

    let installed_marker = issue.mark_or(Mark::installed(is_installed));

    report.marked_item(
        format_args!(
            "{version_label} ({}) - {}{}",
            updates.current_release.release_date.format("%Y-%m-%d"),
            release_notes_url(updates.current_release.version).tone(Tone::Link),
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

    if issue.has_issue() {
        report_error_description(report, &issue);
    }
    Ok(())
}

fn print_available_updates(releases: &ReleaseUpdates, report: &Report) -> anyhow::Result<()> {
    report.header("Available update(s):", HeaderLevel::H2);
    let version_col_width = releases
        .newer_releases
        .iter()
        .map(|rd| rd.version.format_len())
        .max()
        .ok_or_else(|| anyhow!("No releases"))?;

    for release in releases.newer_releases.iter() {
        let issue = release.issue();
        let version_tone = issue.tone_or(Tone::Update);
        let is_installed = release.version.is_editor_installed()?;

        let version = release.version.tone(version_tone);
        let (version, installed) = if is_installed {
            (version.bold(), format!(" > {}", "installed".bold()))
        } else {
            (version, String::new())
        };

        report.marked_item(
            format_args!(
                "{version:<version_col_width$} ({rd}) - {rn}{is}{installed}",
                rd = release.release_date.format("%Y-%m-%d"),
                rn = release_notes_url(release.version).tone(Tone::Link),
                is = issue.issue_tag(),
            ),
            issue.mark(),
        );
    }

    Ok(())
}
