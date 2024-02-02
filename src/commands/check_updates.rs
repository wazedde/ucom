use std::io::Write;
use std::path::Path;

use colored::Colorize;

use crate::commands::terminal_spinner::TerminalSpinner;
use crate::commands::INDENT;
use crate::unity::*;

/// Checks on the Unity website for updates to the version used by the project.
pub fn check_updates(project_dir: &Path, create_report: bool) -> anyhow::Result<()> {
    let project = ProjectPath::try_from(project_dir)?;
    let unity_version = project.unity_version()?;
    let spinner = TerminalSpinner::new(format!(
        "Project uses {unity_version}; checking for updates..."
    ));

    let (project_version_info, updates) = fetch_update_info(unity_version)?;
    drop(spinner);

    if create_report {
        colored::control::set_override(false);
    }

    let mut buf = Vec::new();
    write_project_header(&project, create_report, &mut buf)?;
    writeln!(buf)?;

    write_project_version(
        unity_version,
        project_version_info,
        &updates,
        create_report,
        &mut buf,
    )?;

    if create_report {
        let mut spinner = TerminalSpinner::new("Downloading Unity release notes...");
        for release in updates {
            spinner.update_text(format!(
                "Downloading Unity {} release notes...",
                release.version
            ));

            write_release_notes(&mut buf, &release)?;
        }
        drop(spinner);
        print!("{}", String::from_utf8(buf)?);
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
    project: &ProjectPath,
    create_report: bool,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    if create_report {
        write!(buf, "# ")?;
    }

    let s = format!(
        "Unity updates for: {}",
        project.as_path().to_string_lossy().bold()
    )
    .bold();
    writeln!(buf, "{s}")?;

    if create_report {
        writeln!(buf)?;
    }

    match Settings::from_project(project) {
        Ok(settings) => {
            let ps = settings.player_settings;
            writeln!(buf, "{}Product name:  {}", INDENT, ps.product_name.bold())?;
            writeln!(buf, "{}Company name:  {}", INDENT, ps.company_name.bold())?;
            writeln!(buf, "{}Version:       {}", INDENT, ps.bundle_version.bold())?;
        }

        Err(e) => {
            writeln!(
                buf,
                "{INDENT}{}: {}",
                "No project settings found".yellow(),
                e.to_string().yellow()
            )?;
        }
    }
    Ok(())
}

fn write_project_version(
    project_version: Version,
    project_version_info: Option<ReleaseInfo>,
    updates: &[ReleaseInfo],
    create_report: bool,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let is_installed = project_version.is_editor_installed()?;
    write!(buf, "{}", "Unity editor: ".bold())?;

    let version = match (is_installed, updates.is_empty()) {
        (true, true) => {
            writeln!(buf, "{}", "installed and up to date".green().bold())?;
            project_version.to_string().green()
        }
        (true, false) => {
            writeln!(buf, "{}", "installed and out of date".yellow().bold())?;
            project_version.to_string().yellow()
        }
        (false, true) => {
            writeln!(buf, "{}", "not installed and up to date".red().bold())?;
            project_version.to_string().red()
        }
        (false, false) => {
            writeln!(buf, "{}", "not installed and out of date".red().bold())?;
            project_version.to_string().red()
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
        release_notes_url(project_version).bright_blue()
    )?;

    if is_installed {
        // The editor used by the project is installed, finish the line.
        writeln!(buf)?;
    } else if create_report {
        // The editor used by the project is not installed, and we're writing to a file.
        writeln!(
            buf,
            " > {}",
            project_version_info
                .map(|r| r.installation_url)
                .map_or_else(
                    || "No release info available".into(),
                    |s| format!("[install in Unity HUB]({s})")
                )
        )?;
    } else {
        // The editor used by the project is not installed, and we're not writing to a file.
        writeln!(
            buf,
            " > {}",
            project_version_info
                .map_or_else(
                    || "No release info available".to_string(),
                    |r| r.installation_url.bright_blue().to_string()
                )
                .bold()
        )?;
    }

    Ok(())
}

fn write_available_updates(updates: &[ReleaseInfo], buf: &mut Vec<u8>) -> anyhow::Result<()> {
    writeln!(buf, "{}", "Available update(s):".bold())?;

    let max_len = updates.iter().map(|ri| ri.version.len()).max().unwrap();

    for release in updates {
        let status = if release.version.is_editor_installed()? {
            "installed".bold()
        } else {
            release.installation_url.bright_blue().bold()
        };

        writeln!(
            buf,
            "- {:<max_len$} - {} > {}",
            release.version.to_string().blue().bold(),
            release_notes_url(release.version).bright_blue(),
            status
        )?;
    }

    Ok(())
}

fn write_release_notes(buf: &mut Vec<u8>, release: &ReleaseInfo) -> anyhow::Result<()> {
    let (url, body) = fetch_release_notes(release.version)?;
    let release_notes = extract_release_notes(&body);

    if !release_notes.is_empty() {
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
    }

    Ok(())
}
