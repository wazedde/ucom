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
/// └─ 2021.3.21f1 - Up to date *default for projects
/// ┬─ 2022.2.6f1
/// ├─ 2022.2.10f1 - Update(s) available
/// └─ 2022.2.11f1 - https://unity.com/releases/editor/whats-new/2022.2.11 > unityhub://2022.2.11f1/621cd60d08fd
/// ```
fn print_installed_versions(installed: &[UnityVersion]) -> anyhow::Result<()> {
    let default_version = default_version(installed)?;
    let version_groups = group_minor_versions(installed);
    let max_len = max_version_string_length(&version_groups);

    for group in version_groups {
        for entry in &group {
            print_list_marker(
                entry.version() != group.first().unwrap().version(),
                entry.version() == group.last().unwrap().version(),
            );

            if entry.version() == &default_version {
                println!(
                    "{}",
                    format!("{:<max_len$} *default for projects", entry.version()).bold()
                );
            } else {
                println!("{}", entry.version());
            }
        }
    }
    Ok(())
}

/// Returns the max length of a version string in a list of lists of versions.
fn max_version_string_length(version_groups: &[Vec<VersionType>]) -> usize {
    version_groups
        .iter()
        .flat_map(|f| f.iter())
        .map(|e| e.version().len())
        .max()
        .unwrap()
}

enum VersionType {
    /// Installed version.
    Installed {
        version: UnityVersion,
        /// Is the latest installed version in the minor range.
        is_latest_minor: bool,
    },
    /// Update to installed version.
    Update(ReleaseInfo),
}

impl VersionType {
    fn version(&self) -> &UnityVersion {
        match self {
            VersionType::Installed { version, .. } => version,
            VersionType::Update(ri) => &ri.version,
        }
    }
}

/// Returns list of grouped versions that are in the same minor range.
fn group_minor_versions(installed: &[UnityVersion]) -> Vec<Vec<VersionType>> {
    let mut version_groups = vec![];
    let mut group = vec![];

    let mut iter = installed.iter().peekable();
    while let Some(&version) = iter.next() {
        let is_latest_minor = iter.peek().map_or(true, |v| {
            (v.major, v.minor) != (version.major, version.minor)
        });

        if is_latest_minor {
            // Finished group
            group.push(VersionType::Installed {
                version,
                is_latest_minor,
            });
            version_groups.push(group);

            // Create a new group
            group = vec![];
        } else {
            // In current group
            group.push(VersionType::Installed {
                version,
                is_latest_minor,
            });
        }
    }

    if !group.is_empty() {
        version_groups.push(group);
    }

    version_groups
}

/// Prints list of installed versions and available updates.
/// ```
/// ── 2019.4.40f1 - Up to date
/// ── 2020.3.43f1 - 3 behind 2020.3.46f1 > unityhub://2020.3.46f1/18bc01a066b4
/// ┬─ 2021.3.9f1
/// ├─ 2021.3.16f1
/// ├─ 2021.3.19f1
/// └─ 2021.3.20f1 - Up to date *default for projects
/// ┬─ 2022.2.6f1
/// └─ 2022.2.10f1 - Up to date
/// ```
fn print_updates(installed: &[UnityVersion], available: &Vec<ReleaseInfo>) -> anyhow::Result<()> {
    if available.is_empty() {
        return Err(anyhow!("No update information available."));
    }

    let default_version = default_version(installed)?;
    let mut version_groups = group_minor_versions(installed);

    // Add available updates to groups
    for group in &mut version_groups {
        let latest_installed = *group.last().unwrap().version();

        available
            .iter()
            .filter(|ri| {
                ri.version.major == latest_installed.major
                    && ri.version.minor == latest_installed.minor
                    && ri.version > latest_installed
            })
            .for_each(|ri| {
                group.push(VersionType::Update(ri.clone()));
            });
    }

    let max_len = max_version_string_length(&version_groups);

    for group in version_groups {
        for entry in &group {
            print_list_marker(
                entry.version() != group.first().unwrap().version(),
                entry.version() == group.last().unwrap().version(),
            );

            let last_in_group = entry.version() == group.last().unwrap().version();

            match entry {
                VersionType::Installed {
                    version,
                    is_latest_minor,
                } => {
                    let has_updates = !last_in_group && *is_latest_minor;
                    let is_default = *version == default_version;

                    let mut line = String::new();

                    match (is_latest_minor, has_updates) {
                        (false, _) => line.push_str(&format!("{}", version)),
                        (true, false) => {
                            line.push_str(&format!("{:<max_len$} - Up to date", version));
                        }
                        (true, true) => {
                            line.push_str(&format!("{:<max_len$} - Update(s) available", version));
                        }
                    }

                    if is_default {
                        line.push_str(" *default for projects");
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
/// └─ 2020.3.46f1 - Installed: 2020.3.43f1 - update > unityhub://2020.3.46f1/18bc01a066b4
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
        .map(|ri| ri.version.len())
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
            print_installs(latest, &installed_in_range, max_len);
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

fn print_installs(latest: &ReleaseInfo, installed_in_range: &[UnityVersion], max_len: usize) {
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
                "{:<max_len$} - Installed: {} - update > {}",
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
