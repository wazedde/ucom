use std::env;

use anyhow::anyhow;
use colored::Colorize;
use itertools::Itertools;
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
/// ── 2020.3.46f1 - https://unity.com/releases/editor/whats-new/2020.3.46
/// ┬─ 2021.3.19f1 - https://unity.com/releases/editor/whats-new/2021.3.19
/// ├─ 2021.3.22f1 - https://unity.com/releases/editor/whats-new/2021.3.22
/// └─ 2021.3.23f1 - https://unity.com/releases/editor/whats-new/2021.3.23 *default for projects
/// ── 2022.2.15f1 - https://unity.com/releases/editor/whats-new/2022.2.15
/// ── 2023.1.0b12 - https://unity.com/releases/editor/beta/2023.1.0b12
/// ── 2023.2.0a10 - https://unity.com/releases/editor/alpha/2023.2.0a10
/// ```
fn print_installed_versions(installed: &[UnityVersion]) -> anyhow::Result<()> {
    let default_version = default_version(installed)?;
    let version_groups = group_minor_versions(installed);
    let max_len = max_version_string_length(&version_groups);

    for group in version_groups {
        for entry in &group {
            print_list_marker(
                entry.version == group.first().unwrap().version,
                entry.version == group.last().unwrap().version,
            );

            let line = format!(
                "{:<max_len$} - {}",
                entry.version.to_string(),
                release_notes_url(entry.version).bright_blue()
            );

            if entry.version == default_version {
                println!("{} {}", line.bold(), "*default for projects".bold());
            } else {
                println!("{line}");
            }
        }
    }
    Ok(())
}

/// Prints list of installed versions and available updates.
/// ```
/// ┬─ 2020.3.46f1 - Update(s) available
/// └─ 2020.3.47f1 - https://unity.com/releases/editor/whats-new/2020.3.47 > unityhub://2020.3.47f1/5ef4f5b5e2d4
/// ┬─ 2021.3.19f1
/// ├─ 2021.3.22f1
/// └─ 2021.3.23f1 - Up to date *default for projects
/// ── 2022.2.15f1 - Up to date
/// ── 2023.1.0b12 - No Beta update info available
/// ── 2023.2.0a10 - No Alpha update info available
/// ```
fn print_updates(installed: &[UnityVersion], available: &Vec<ReleaseInfo>) -> anyhow::Result<()> {
    if available.is_empty() {
        return Err(anyhow!("No update information available."));
    }

    let default_version = default_version(installed)?;
    let version_groups = collect_update_info(installed, available);
    let max_len = max_version_string_length(&version_groups);

    let print_line = |line: &str, is_default: bool| {
        if is_default {
            println!("{} {}", line.bold(), "*default for projects".bold());
        } else {
            println!("{line}");
        }
    };

    for group in version_groups {
        for info in &group {
            print_list_marker(
                info.version == group.first().unwrap().version,
                info.version == group.last().unwrap().version,
            );

            match &info.v_type {
                VersionType::HasLaterInstalled => {
                    print_line(&info.version.to_string(), info.version == default_version);
                }
                VersionType::LatestInstalled => {
                    let last_in_group = info.version == group.last().unwrap().version;

                    let line = if last_in_group {
                        format!(
                            "{:<max_len$} - {}",
                            info.version.to_string().green(),
                            "Up to date"
                        )
                    } else {
                        format!(
                            "{:<max_len$} - {}",
                            info.version.to_string(),
                            "Update(s) available".blue().bold()
                        )
                    };

                    print_line(&line, info.version == default_version);
                }
                VersionType::UpdateToLatest(release_info) => {
                    println!(
                        "{:<max_len$} - {} > {}",
                        release_info.version.to_string().blue().bold(),
                        release_notes_url(release_info.version).bright_blue().bold(),
                        release_info.installation_url.bright_blue().bold(),
                    );
                }
                VersionType::NoReleaseInfo => {
                    let line = format!(
                        "{:<max_len$} - {}",
                        info.version.to_string(),
                        format!(
                            "No {} update info available",
                            info.version.build_type.as_full_str()
                        )
                        .bright_black()
                    );

                    print_line(&line, info.version == default_version);
                }
            }
        }
    }

    Ok(())
}

/// Groups installed versions by major.minor version
/// and collects update information for each installed version.
fn collect_update_info(
    installed: &[UnityVersion],
    available: &[ReleaseInfo],
) -> Vec<Vec<VersionInfo>> {
    let mut version_groups = group_minor_versions(installed);

    // Add available updates to groups
    for group in &mut version_groups {
        let latest_installed_version = group.last().expect("Group cannot be empty").version;

        let has_releases = available.iter().any(|ri| {
            ri.version.major == latest_installed_version.major
                && ri.version.minor == latest_installed_version.minor
        });

        if has_releases {
            // Add update info to group (if there are any)
            available
                .iter()
                .filter(|ri| {
                    ri.version.major == latest_installed_version.major
                        && ri.version.minor == latest_installed_version.minor
                        && ri.version > latest_installed_version
                })
                .for_each(|ri| {
                    group.push(VersionInfo {
                        version: ri.version,
                        v_type: VersionType::UpdateToLatest(ri.clone()),
                    });
                });
        } else {
            // No release info available for this minor version, pop it off...
            let v = group.pop().unwrap().version;
            // ...and add a NoReleaseInfo entry
            group.push(VersionInfo {
                version: v,
                v_type: VersionType::NoReleaseInfo,
            });
        }
    }
    version_groups
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
    let minor_releases = latest_minor_releases(available, partial_version);

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
            Some(latest.version.major) != previous_major,
            is_last_in_range,
        );

        previous_major = Some(latest.version.major);

        // Find all installed versions in the same range as the latest version.
        let installed_in_range = installed
            .iter()
            .filter(|v| v.major == latest.version.major && v.minor == latest.version.minor)
            .copied()
            .collect_vec();

        if installed_in_range.is_empty() {
            // No installed versions in the range.
            println!(
                "{:<max_len$} > {}",
                latest.version.to_string(),
                latest.installation_url.bright_blue()
            );
        } else {
            print_installs_line(latest, &installed_in_range, max_len);
        }
    }
}

fn print_installs_line(latest: &ReleaseInfo, installed_in_range: &[UnityVersion], max_len: usize) {
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

    let line = if is_up_to_date {
        format!(
            "{:<max_len$} - Installed: {}",
            latest.version.to_string(),
            joined_versions
        )
        .bold()
    } else {
        format!(
            "{:<max_len$} - Installed: {} - update > {}",
            latest.version.to_string().blue(),
            joined_versions.blue(),
            latest.installation_url.bright_blue()
        )
        .blue()
        .bold()
    };
    println!("{line}");
}

struct VersionInfo {
    version: UnityVersion,
    v_type: VersionType,
}

enum VersionType {
    HasLaterInstalled,
    LatestInstalled,
    UpdateToLatest(ReleaseInfo),
    NoReleaseInfo,
}

/// Returns the max length of a version string in a list of lists of versions.
fn max_version_string_length(version_groups: &[Vec<VersionInfo>]) -> usize {
    version_groups
        .iter()
        .flatten()
        .map(|e| e.version.len())
        .max()
        .unwrap()
}

/// Returns list of grouped versions that are in the same minor range.
fn group_minor_versions(installed: &[UnityVersion]) -> Vec<Vec<VersionInfo>> {
    let mut version_groups = vec![];
    let mut group = vec![];

    for (i, &version) in installed.iter().enumerate() {
        let next_version = installed.get(i + 1);
        let is_latest_minor = next_version.map_or(true, |v| {
            (v.major, v.minor) != (version.major, version.minor)
        });

        let v_type = if is_latest_minor {
            VersionType::LatestInstalled
        } else {
            VersionType::HasLaterInstalled
        };

        group.push(VersionInfo { version, v_type });

        // Finished group
        if is_latest_minor {
            version_groups.push(std::mem::take(&mut group));
        }
    }

    // Add the last group
    if !group.is_empty() {
        version_groups.push(group);
    }

    version_groups
}

fn latest_minor_releases<'a>(
    available: &'a [ReleaseInfo],
    partial_version: Option<&str>,
) -> Vec<&'a ReleaseInfo> {
    available
        .iter()
        .filter(|r| partial_version.map_or(true, |p| r.version.to_string().starts_with(p)))
        .map(|r| (r.version.major, r.version.minor))
        .sorted_unstable()
        .dedup()
        .filter_map(|(major, minor)| {
            available
                .iter()
                .filter(|r| r.version.major == major && r.version.minor == minor)
                .max()
        })
        .collect()
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
fn print_list_marker(is_first: bool, is_last: bool) {
    print!("{} ", list_marker(is_first, is_last));
}

/// Returns the list marker for the current item.
fn list_marker(is_first: bool, is_last: bool) -> &'static str {
    match (is_first, is_last) {
        (true, true) => "──",
        (true, false) => "┬─",
        (false, false) => "├─",
        (false, true) => "└─",
    }
}
