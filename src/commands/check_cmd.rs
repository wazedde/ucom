use std::io::Write;
use std::path::Path;

use yansi::Paint;

use crate::commands::term_stat::TermStat;
use crate::commands::{writeln_b, INDENT};
use crate::unity::release_api_data::ReleaseData;
use crate::unity::*;

/// Checks on the Unity website for updates to the version used by the project.
pub(crate) fn check_updates(project_dir: &Path, create_report: bool) -> anyhow::Result<()> {
    let project = ProjectPath::try_from(project_dir)?;
    let unity_version = project.unity_version()?;

    let (project_version_info, updates) = {
        let _ts = TermStat::new("Checking", &format!("for updates to {unity_version}"));
        fetch_update_info(unity_version)?
    };

    if create_report {
        yansi::disable();
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
        let ts = TermStat::new("Downloading", "Unity release notes...");
        for release in updates {
            ts.reprint(
                "Downloading",
                &format!("Unity {} release notes...", release.version),
            );

            write_release_notes(&mut buf, &release)?;
        }
        drop(ts);
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

    writeln_b!(buf, "Unity updates for: {}", project.as_path().display())?;

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
                e.yellow()
            )?;
        }
    }
    Ok(())
}

fn write_project_version(
    project_version: Version,
    project_version_info: Option<ReleaseData>,
    updates: &[ReleaseData],
    create_report: bool,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let is_installed = project_version.is_editor_installed()?;
    write!(buf, "{}", "Unity editor: ".bold())?;

    let version = match (is_installed, updates.is_empty()) {
        (true, true) => {
            writeln!(buf, "{}", "installed and up to date".green().bold())?;
            project_version.green()
        }
        (true, false) => {
            writeln!(buf, "{}", "installed and out of date".yellow().bold())?;
            project_version.yellow()
        }
        (false, true) => {
            writeln!(buf, "{}", "not installed and up to date".red().bold())?;
            project_version.red()
        }
        (false, false) => {
            writeln!(buf, "{}", "not installed and out of date".red().bold())?;
            project_version.red()
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
                .map(|r| r.unity_hub_deep_link)
                .map_or_else(
                    || "No release info available".into(),
                    |s| format!("[install in Unity HUB]({s})"),
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
                    |r| r.unity_hub_deep_link.bright_blue().to_string(),
                )
                .bold()
        )?;
    }

    Ok(())
}

fn write_available_updates(updates: &[ReleaseData], buf: &mut Vec<u8>) -> anyhow::Result<()> {
    writeln_b!(buf, "Available update(s):")?;
    let max_len = updates.iter().map(|ri| ri.version.len()).max().unwrap();

    for release in updates {
        let status = if release.version.is_editor_installed()? {
            "installed".bold()
        } else {
            release.unity_hub_deep_link.as_str().bright_blue().bold()
        };

        writeln!(
            buf,
            "- {:<max_len$} - {} > {}",
            release.version.blue().bold(),
            release_notes_url(release.version).bright_blue(),
            status
        )?;
    }

    Ok(())
}

fn write_release_notes(buf: &mut Vec<u8>, release: &ReleaseData) -> anyhow::Result<()> {
    let (url, body) = fetch_release_notes(release.version)?;
    let release_notes = extract_release_notes(&body);

    if !release_notes.is_empty() {
        writeln!(buf)?;
        writeln!(buf, "## Release notes for [{}]({url})", release.version)?;
        writeln!(buf)?;
        writeln!(
            buf,
            "[install in Unity HUB]({})",
            release.unity_hub_deep_link
        )?;

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
