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

    if let Some(path) = report_path {
        validate_report_path(path)?;
    }

    let version = version_used_by_project(&project_dir)?;

    let mut spinner = Spinner::new(
        spinners::Dots,
        format!("Project uses {version}; checking for updates..."),
        None,
    );

    if report_path.is_some() {
        // Disable colored output when writing to a file.
        colored::control::set_override(false);
    }

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
        format!("Unity updates for {product_name}").bold()
    )?;

    drop(product_name);

    if report_path.is_some() {
        writeln!(buf)?;
    }

    writeln!(
        buf,
        "    Directory:            {}",
        project_dir.to_string_lossy().bold()
    )?;

    write!(
        buf,
        "    Project uses version: {}",
        version.to_string().bold()
    )?;

    if is_editor_installed(version)? {
        writeln!(buf)?;
    } else {
        writeln!(buf, " {}", "*not installed".red().bold())?;
    }

    let updates = request_patch_updates_for(version)?;
    if updates.is_empty() {
        writeln!(
            buf,
            "    Already uses the latest release in the {}.{}.x range",
            version.year, version.point
        )?;
        spinner.clear();
        print!("{}", String::from_utf8(buf)?);
        return Ok(());
    }

    let latest = updates.last().unwrap();
    writeln!(
        buf,
        "    Update available:     {}",
        latest.version.to_string().yellow().bold()
    )?;

    if report_path.is_none() {
        spinner.clear();
        print!("{}", String::from_utf8(buf)?);
        return Ok(());
    }

    for release in updates {
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

fn fetch_release_notes(buf: &mut Vec<u8>, release: &ReleaseInfo) -> anyhow::Result<()> {
    let (url, body) = request_release_notes(release.version)?;
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
