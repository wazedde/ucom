use anyhow::anyhow;
use itertools::Itertools;
use yansi::Paint;

use crate::cli::ListType;
use crate::commands::term_stat::TermStat;
use crate::commands::ColoredStringIf;
use crate::unity::installed::InstalledVersions;
use crate::unity::*;

/// Lists installed Unity versions.
pub fn list_versions(list_type: ListType, partial_version: Option<&str>) -> anyhow::Result<()> {
    let (dir, versions) = InstalledVersions::get()?;

    match list_type {
        ListType::Installed => {
            let matching_versions = versions.prune(partial_version)?;
            let line = format!(
                "Unity versions in: {} (*=default for new projects)",
                dir.display(),
            );
            println!("{}", line.bold());
            print_installed_versions(&matching_versions)?;
            Ok(())
        }
        ListType::Updates => {
            let matching_versions = versions.prune(partial_version)?;
            let line = format!(
                "Updates for Unity versions in: {} (*=default for new projects)",
                dir.display(),
            );
            println!("{}", line.bold());
            let ts = TermStat::new("Downloading", "release data...");
            let releases = fetch_unity_editor_releases()?;
            drop(ts);
            print_updates(&matching_versions, &releases)?;
            Ok(())
        }
        ListType::Latest => {
            let matching_versions: Vec<_> = versions
                .prune(partial_version)
                .map(|v| v.into())
                .unwrap_or_default();
            println!("{}", "Latest available minor releases".bold());
            let ts = TermStat::new("Downloading", "release data...");
            let releases = fetch_unity_editor_releases()?;
            drop(ts);
            print_latest_versions(&matching_versions, &releases, partial_version);
            Ok(())
        }
        ListType::All => {
            let matching_versions: Vec<_> = versions
                .prune(partial_version)
                .map(|v| v.into())
                .unwrap_or_default();

            println!("{}", "Available releases".bold());
            let ts = TermStat::new("Downloading", "release data...");
            let releases = fetch_unity_editor_releases()?;
            drop(ts);
            print_available_versions(&matching_versions, &releases, partial_version);
            Ok(())
        }
    }
}

/// Prints list of installed versions.
/// ```
/// ── 2020.3.46f1  - https://unity.com/releases/editor/whats-new/2020.3.46
/// ┬─ 2021.3.19f1  - https://unity.com/releases/editor/whats-new/2021.3.19
/// ├─ 2021.3.22f1  - https://unity.com/releases/editor/whats-new/2021.3.22
/// └─ 2021.3.23f1* - https://unity.com/releases/editor/whats-new/2021.3.23
/// ── 2022.2.15f1  - https://unity.com/releases/editor/whats-new/2022.2.15
/// ── 2023.1.0b12  - https://unity.com/releases/editor/beta/2023.1.0b12
/// ── 2023.2.0a10  - https://unity.com/releases/editor/alpha/2023.2.0a10
/// ```
fn print_installed_versions(installed: &InstalledVersions) -> anyhow::Result<()> {
    let default_version = installed.default_version()?;
    let version_groups = group_minor_versions(installed.as_ref());
    let max_len = max_version_string_length(&version_groups).max(default_version.len() + 1);

    for group in version_groups {
        for vi in &group {
            print_list_marker(
                vi.version == group.first().unwrap().version,
                vi.version == group.last().unwrap().version,
            );

            let mut version_str = vi.version.to_string();
            if vi.version == default_version {
                version_str.push('*');
            }

            let line = format!(
                "{:<max_len$} - {}",
                version_str,
                release_notes_url(vi.version).bright_blue()
            );
            println!("{}", line.bold_if(vi.version == default_version));
        }
    }
    Ok(())
}

/// Prints list of installed versions and available updates.
/// ```
/// ┬─ 2020.3.46f1  - Update(s) available
/// └─ 2020.3.47f1  - https://unity.com/releases/editor/whats-new/2020.3.47 > unityhub://2020.3.47f1/5ef4f5b5e2d4
/// ┬─ 2021.3.19f1
/// ├─ 2021.3.22f1
/// └─ 2021.3.23f1* - Up to date
/// ── 2022.2.15f1  - Up to date
/// ── 2023.1.0b12  - No Beta update info available
/// ── 2023.2.0a10  - No Alpha update info available
/// ```
fn print_updates(installed: &InstalledVersions, available: &[ReleaseInfo]) -> anyhow::Result<()> {
    if available.is_empty() {
        return Err(anyhow!("No update information available."));
    }

    let default_version = installed.default_version()?;
    let version_groups = collect_update_info(installed, available).unwrap();
    let max_len = max_version_string_length(&version_groups).max(default_version.len() + 1);

    for group in version_groups {
        for vi in &group {
            print_list_marker(
                vi.version == group.first().unwrap().version,
                vi.version == group.last().unwrap().version,
            );

            let mut version_str = vi.version.to_string();
            if vi.version == default_version {
                version_str.push('*');
            }

            let line = match &vi.v_type {
                VersionType::HasLaterInstalled => version_str,
                VersionType::LatestInstalled => {
                    let last_in_group = vi.version == group.last().unwrap().version;
                    if last_in_group {
                        format!("{:<max_len$} - Up to date", version_str.green())
                    } else {
                        format!("{:<max_len$} - Update(s) available", version_str)
                    }
                }
                VersionType::UpdateToLatest(release_info) => {
                    format!(
                        "{:<max_len$} - {} > {}",
                        release_info.version.to_string().blue().bold(),
                        release_notes_url(release_info.version).bright_blue().bold(),
                        release_info.installation_url.bright_blue().bold(),
                    )
                }
                VersionType::NoReleaseInfo => {
                    format!(
                        "{:<max_len$} - {}",
                        version_str,
                        format!("No {} update info available", vi.version.build_type,)
                            .bright_black()
                    )
                }
            };
            println!("{}", line.bold_if(vi.version == default_version));
        }
    }
    Ok(())
}

/// Groups installed versions by `major.minor` version
/// and collects update information for each installed version.
fn collect_update_info(
    installed: &InstalledVersions,
    available: &[ReleaseInfo],
) -> anyhow::Result<Vec<Vec<VersionInfo>>> {
    let mut version_groups = group_minor_versions(installed.as_ref());

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
    Ok(version_groups)
}

/// Prints list of latest available Unity versions.
/// ```
/// ...
/// ┬─ 2019.1.14f1 > unityhub://2019.1.14f1/148b5891095a
/// ├─ 2019.2.21f1 > unityhub://2019.2.21f1/9d528d026557
/// ├─ 2019.3.15f1 > unityhub://2019.3.15f1/59ff3e03856d
/// └─ 2019.4.40f1 > unityhub://2019.4.40f1/ffc62b691db5
/// ┬─ 2020.1.17f1 > unityhub://2020.1.17f1/9957aee8edc2
/// ├─ 2020.2.7f1  > unityhub://2020.2.7f1/c53830e277f1
/// └─ 2020.3.48f1 > unityhub://2020.3.48f1/b805b124c6b7
/// ┬─ 2021.1.28f1 > unityhub://2021.1.28f1/f3f9dc10f3dd
/// ├─ 2021.2.19f1 > unityhub://2021.2.19f1/602ecdbb2fb0
/// └─ 2021.3.29f1 - Installed: 2021.3.26f1, 2021.3.29f1
/// ┬─ 2022.1.24f1 > unityhub://2022.1.24f1/709dddfb713f
/// ├─ 2022.2.21f1 > unityhub://2022.2.21f1/4907324dc95b
/// └─ 2022.3.5f1  - Installed: 2022.3.5f1
/// ── 2023.1.6f1  > unityhub://2023.1.6f1/964b2488c462
/// ```
fn print_latest_versions(
    installed: &[Version],
    available: &[ReleaseInfo],
    partial_version: Option<&str>,
) {
    // Get the latest version of each range.
    let minor_releases = latest_minor_releases(available, partial_version);

    if minor_releases.is_empty() {
        println!(
            "No releases available that match `{}`",
            partial_version.unwrap_or("*")
        );
        return;
    }

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

/// Prints list of available Unity versions.
/// ```
/// ...
/// ├─ 2021.1.4f1  - https://unity.com/releases/editor/whats-new/2021.1.4 > unityhub://2021.1.4f1/4cd64a618c1b
/// ├─ 2021.1.5f1  - https://unity.com/releases/editor/whats-new/2021.1.5 > unityhub://2021.1.5f1/3737af19df53
/// ├─ 2021.1.6f1  - https://unity.com/releases/editor/whats-new/2021.1.6 > unityhub://2021.1.6f1/c0fade0cc7e9
/// ├─ 2021.1.7f1  - https://unity.com/releases/editor/whats-new/2021.1.7 > unityhub://2021.1.7f1/d91830b65d9b
/// ├─ 2021.1.9f1  - https://unity.com/releases/editor/whats-new/2021.1.9 > unityhub://2021.1.9f1/7a790e367ab3
/// ├─ 2021.1.10f1 - https://unity.com/releases/editor/whats-new/2021.1.10 > unityhub://2021.1.10f1/b15f561b2cef
/// ├─ 2021.1.11f1 - https://unity.com/releases/editor/whats-new/2021.1.11 > unityhub://2021.1.11f1/4d8c25f7477e
/// ├─ 2021.1.12f1 - https://unity.com/releases/editor/whats-new/2021.1.12 > unityhub://2021.1.12f1/afcadd793de6
/// ├─ 2021.1.13f1 - https://unity.com/releases/editor/whats-new/2021.1.13 > unityhub://2021.1.13f1/a03098edbbe0
/// ├─ 2021.1.14f1 - https://unity.com/releases/editor/whats-new/2021.1.14 > unityhub://2021.1.14f1/51d2f824827f
/// ...
/// ```
fn print_available_versions(
    installed: &[Version],
    available: &[ReleaseInfo],
    partial_version: Option<&str>,
) {
    let releases = available
        .iter()
        .filter(|r| partial_version.map_or(true, |p| r.version.to_string().starts_with(p)))
        .sorted_unstable()
        .dedup()
        .collect_vec();

    if releases.is_empty() {
        println!(
            "No releases available that match `{}`",
            partial_version.unwrap_or("*")
        );
        return;
    }

    let versions = releases.iter().map(|r| r.version).collect_vec();
    let version_groups = group_minor_versions(&versions);
    let max_len = max_version_string_length(&version_groups);

    for group in version_groups {
        for vi in &group {
            print_list_marker(
                vi.version == group.first().unwrap().version,
                vi.version == group.last().unwrap().version,
            );

            let is_installed = installed.contains(&vi.version);
            let version_str = vi.version.to_string();

            if is_installed {
                let line = format!(
                    "{:<max_len$} - {} > installed",
                    version_str.green(),
                    release_notes_url(vi.version).bright_blue()
                );
                println!("{}", line.bold());
            } else {
                let ri = releases
                    .iter()
                    .find(|p| p.version == vi.version)
                    .expect("Could not find release info for version");

                let line = format!(
                    "{:<max_len$} - {} > {}",
                    version_str,
                    release_notes_url(vi.version).bright_blue(),
                    ri.installation_url.bright_blue()
                );
                println!("{}", line);
            }
        }
    }
}

fn print_installs_line(latest: &ReleaseInfo, installed_in_range: &[Version], max_len: usize) {
    let is_up_to_date = installed_in_range
        .last()
        .filter(|&v| v == &latest.version)
        .is_some()
        || installed_in_range // Special case for when installed version is newer than the latest.
            .last()
            .map_or(false, |&v| v > latest.version);

    // Concatenate the installed versions for printing.
    let joined_versions = installed_in_range
        .iter()
        .map(ToString::to_string)
        .collect_vec()
        .join(", ");

    let line = if is_up_to_date {
        format!(
            "{:<max_len$} - Installed: {}",
            latest.version.to_string().green(),
            joined_versions
        )
    } else {
        format!(
            "{:<max_len$} - Installed: {} - update > {}",
            latest.version.to_string().blue(),
            joined_versions,
            latest.installation_url.bright_blue()
        )
    };
    println!("{}", line.bold());
}

struct VersionInfo {
    version: Version,
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
        .unwrap_or(0)
}

/// Returns list of grouped versions that are in the same minor range.
fn group_minor_versions(installed: &[Version]) -> Vec<Vec<VersionInfo>> {
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
