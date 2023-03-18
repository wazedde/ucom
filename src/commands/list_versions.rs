use std::env;

use anyhow::anyhow;
use colored::Colorize;
use spinoff::{spinners, Spinner};

use crate::cli::{ListType, ENV_DEFAULT_VERSION};
use crate::unity::*;

/// Lists installed Unity versions.
pub fn list_versions(list_type: ListType, partial_version: Option<&str>) -> anyhow::Result<()> {
    let dir = editor_parent_dir()?;
    let versions = available_unity_versions(&dir)?;
    let matching_versions = matching_versions(versions, partial_version)?;

    match list_type {
        ListType::Installed => {
            println!("{}", format!("Unity versions in: {}", dir.display()).bold());
            print_installed_versions(&matching_versions)?;
        }
        ListType::Updates => {
            println!(
                "{}",
                format!("Updates for Unity versions in: {}", dir.display()).bold()
            );
            let spinner = Spinner::new(spinners::Dots, "Downloading release data...", None);
            let releases = request_unity_releases()?;
            spinner.clear();
            print_updates(&matching_versions, &releases)?;
        }
        ListType::Latest => {
            println!("{}", "Latest available minor releases".bold());
            let spinner = Spinner::new(spinners::Dots, "Downloading release data...", None);
            let releases = request_unity_releases()?;
            spinner.clear();
            print_latest_versions(&matching_versions, &releases, partial_version);
        }
    }

    Ok(())
}

/// Prints list of installed versions.
/// ```
/// ── 2019.4.40f1 - Up to date
/// ┬─ 2020.3.43f1 - Update(s) available
/// ├─ 2020.3.44f1 - https://unity.com/releases/editor/whats-new/2020.3.44 > unityhub://2020.3.44f1/7f159b6136da
/// ├─ 2020.3.45f1 - https://unity.com/releases/editor/whats-new/2020.3.45 > unityhub://2020.3.45f1/660cd1701bd5
/// └─ 2020.3.46f1 - https://unity.com/releases/editor/whats-new/2020.3.46 > unityhub://2020.3.46f1/18bc01a066b4
/// ┬─ 2021.3.19f1
/// └─ 2021.3.21f1 - Up to date *default for new projects
/// ┬─ 2022.2.6f1
/// ├─ 2022.2.10f1 - Update(s) available
/// └─ 2022.2.11f1 - https://unity.com/releases/editor/whats-new/2022.2.11 > unityhub://2022.2.11f1/621cd60d08fd
/// ```
fn print_installed_versions(installed: &[UnityVersion]) -> anyhow::Result<()> {
    let default_version = default_version(installed)?;
    let grouped = group_minor_versions(installed);
    let max_len = max_version_string_length(&grouped);

    for minor in grouped {
        for entry in &minor {
            print_list_marker(
                entry.version() != minor.first().unwrap().version(),
                entry.version() == minor.last().unwrap().version(),
            );

            if entry.version() == &default_version {
                println!(
                    "{}",
                    format!("{:<max_len$} *default for new projects", entry.version()).bold()
                );
            } else {
                println!("{}", entry.version());
            }
        }
    }
    Ok(())
}

/// Returns the max length of a version string in a list of lists of versions.
fn max_version_string_length(vec: &[Vec<VersionType>]) -> usize {
    vec.iter()
        .flat_map(|f| f.iter())
        .map(|e| e.version().to_string().len())
        .max()
        .unwrap()
}

struct InstallInfo {
    /// Installed version.
    version: UnityVersion,
    /// Is the latest installed version in the minor range.
    is_latest: bool,
}

impl InstallInfo {
    fn new(version: UnityVersion, is_latest: bool) -> Self {
        Self { version, is_latest }
    }
}

enum VersionType {
    /// Installed version.
    Installed(InstallInfo),
    /// Update to installed version.
    Update(ReleaseInfo),
}

impl VersionType {
    fn new_installed(version: UnityVersion, is_latest: bool) -> VersionType {
        VersionType::Installed(InstallInfo::new(version, is_latest))
    }

    fn new_update(version: ReleaseInfo) -> VersionType {
        VersionType::Update(version)
    }

    fn version(&self) -> &UnityVersion {
        match self {
            VersionType::Installed(v) => &v.version,
            VersionType::Update(v) => &v.version,
        }
    }
}

/// Returns list of lists of versions grouped by minor version.
fn group_minor_versions(installed: &[UnityVersion]) -> Vec<Vec<VersionType>> {
    let mut versions = vec![];
    let mut minor_range = vec![];
    let mut current_range = None;

    let mut iter = installed.iter().peekable();
    while let Some(&version) = iter.next() {
        let current_minor = (version.major, version.minor);
        let is_latest = iter.peek().map_or(true, |v| {
            let next_minor = (v.major, v.minor);
            next_minor != current_minor
        });

        if current_range == Some(current_minor) {
            // In same minor range
            minor_range.push(VersionType::new_installed(version, is_latest));
        } else {
            // New minor range
            current_range = Some(current_minor);
            if !minor_range.is_empty() {
                versions.push(minor_range);
            }
            minor_range = vec![VersionType::new_installed(version, is_latest)];
        }
    }

    if !minor_range.is_empty() {
        versions.push(minor_range);
    }

    versions
}

/// Prints list of installed versions and available updates.
/// ```
/// ── 2019.4.40f1 - Up to date
/// ── 2020.3.43f1 - 3 behind 2020.3.46f1 > unityhub://2020.3.46f1/18bc01a066b4
/// ┬─ 2021.3.9f1
/// ├─ 2021.3.16f1
/// ├─ 2021.3.19f1
/// └─ 2021.3.20f1 - Up to date *default for new projects
/// ┬─ 2022.2.6f1
/// └─ 2022.2.10f1 - Up to date
/// ```
fn print_updates(installed: &[UnityVersion], available: &Vec<ReleaseInfo>) -> anyhow::Result<()> {
    if available.is_empty() {
        return Err(anyhow!("No update information available."));
    }

    let default_version = default_version(installed)?;
    let mut grouped = group_minor_versions(installed);

    for minor in &mut grouped {
        let latest_installed = *minor.last().unwrap().version();

        let updates = available.iter().filter(|v| {
            v.version.major == latest_installed.major
                && v.version.minor == latest_installed.minor
                && v.version > latest_installed
        });

        for update in updates {
            minor.push(VersionType::new_update(update.clone()));
        }
    }

    let max_len = max_version_string_length(&grouped);

    for minor in grouped {
        for entry in &minor {
            print_list_marker(
                entry.version() != minor.first().unwrap().version(),
                entry.version() == minor.last().unwrap().version(),
            );

            let last_in_group = entry.version() == minor.last().unwrap().version();

            match entry {
                VersionType::Installed(installed) => {
                    let has_updates = !last_in_group && installed.is_latest;
                    let is_default = installed.version == default_version;

                    let mut line = String::new();

                    match (installed.is_latest, has_updates) {
                        (false, _) => line.push_str(&format!("{}", installed.version)),
                        (true, false) => {
                            line.push_str(&format!("{:<max_len$} - Up to date", installed.version));
                        }
                        (true, true) => line.push_str(&format!(
                            "{:<max_len$} - Update(s) available",
                            installed.version
                        )),
                    }

                    if is_default {
                        line.push_str(" *default for new projects");
                        println!("{}", line.bold());
                    } else {
                        println!("{}", line);
                    }
                }
                VersionType::Update(update) => {
                    println!(
                        "{:<max_len$} - {} > {}",
                        update.version.to_string().yellow().bold(),
                        release_notes_url(update.version),
                        update.installation_url.bold(),
                    );
                }
            }
        }
    }
    Ok(())
}

/// Prints list of latest available Unity versions.
/// ```
/// ┬─ 2019.1.14f1
/// ├─ 2019.2.21f1
/// ├─ 2019.3.15f1
/// └─ 2019.4.40f1 - Installed: 2019.4.40f1
/// ┬─ 2020.1.17f1
/// ├─ 2020.2.7f1
/// └─ 2020.3.46f1 - Installed: 2020.3.43f1 (update > unityhub://2020.3.46f1/18bc01a066b4)
/// ┬─ 2021.1.28f1
/// ├─ 2021.2.19f1
/// └─ 2021.3.20f1 - Installed: 2021.3.9f1, 2021.3.16f1, 2021.3.19f1, 2021.3.20f1
/// ┬─ 2022.1.24f1
/// └─ 2022.2.10f1 - Installed: 2022.2.6f1, 2022.2.10f1
/// ```
fn print_latest_versions(
    installed: &[UnityVersion],
    available: &[ReleaseInfo],
    partial_version: Option<&str>,
) {
    // Get the latest version of each range.
    let minor_releases: Vec<_> = latest_minor_releases(available, partial_version);

    let max_len = minor_releases
        .iter()
        .map(|ri| ri.version.to_string().len())
        .max()
        .unwrap_or(0);

    let mut previous_major = None;
    let mut iter = minor_releases.iter().peekable();

    while let Some(latest) = iter.next() {
        let is_last_in_range = iter
            .peek()
            .map_or(true, |v| v.version.major != latest.version.major);

        print_list_marker(
            Some(latest.version.major) == previous_major,
            is_last_in_range,
        );

        previous_major = Some(latest.version.major);

        // Find all installed versions in the same range as the latest version.
        let installed_in_range: Vec<_> = installed
            .iter()
            .filter(|v| v.major == latest.version.major && v.minor == latest.version.minor)
            .copied()
            .collect();

        if installed_in_range.is_empty() {
            // No installed versions in the range.
            println!("{}", latest.version);
        } else {
            print_installs(latest, installed_in_range, max_len)
        }
    }
}

fn latest_minor_releases<'a>(
    available: &'a [ReleaseInfo],
    partial_version: Option<&str>,
) -> Vec<&'a ReleaseInfo> {
    let mut available_releases: Vec<_> = available
        .iter()
        .filter(|r| partial_version.map_or(true, |p| r.version.to_string().starts_with(p)))
        .map(|r| (r.version.major, r.version.minor))
        .collect();

    available_releases.sort_unstable();
    available_releases.dedup();

    available_releases
        .iter()
        .filter_map(|&(major, minor)| {
            available
                .iter()
                .filter(|r| r.version.major == major && r.version.minor == minor)
                .max()
        })
        .collect()
}

fn print_installs(latest: &ReleaseInfo, installed_in_range: Vec<UnityVersion>, max_len: usize) {
    let is_up_to_date = installed_in_range
        .last()
        .filter(|&v| v == &latest.version)
        .is_some()
        || installed_in_range // Special case for when installed version is newer than latest.
            .last()
            .map_or(false, |&v| v > latest.version);

    // Concatenate the installed versions for printing.
    let joined_versions = installed_in_range
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    if is_up_to_date {
        println!(
            "{}",
            format!(
                "{:<max_len$} - Installed: {}",
                latest.version, joined_versions
            )
            .bold()
        );
    } else {
        println!(
            "{}",
            format!(
                "{:<max_len$} - Installed: {} (update > {}),",
                latest.version, joined_versions, latest.installation_url
            )
            .yellow()
            .bold()
        );
    }
}

/// Returns the default version ucom uses for new Unity projects.
fn default_version(installed: &[UnityVersion]) -> anyhow::Result<UnityVersion> {
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
    Ok(default_version)
}

/// Prints the list marker for the current item.
fn print_list_marker(is_continuation: bool, is_last: bool) {
    print!("{} ", list_marker(is_continuation, is_last));
}

/// Returns the list marker for the current item.
fn list_marker(is_continuation: bool, is_last: bool) -> &'static str {
    match (is_continuation, is_last) {
        (false, true) => "──",
        (false, false) => "┬─",
        (true, false) => "├─",
        (true, true) => "└─",
    }
}
