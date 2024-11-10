use anyhow::anyhow;
use crossterm::style::Stylize;
use itertools::Itertools;
use yansi::Paint;

use crate::cli::ListType;
use crate::commands::term_stat::TermStat;
use crate::commands::{println_b, println_b_if};
use crate::unity::installed::VersionList;
use crate::unity::non_empty_vec::NonEmptyVec;
use crate::unity::release_api_data::ReleaseData;
use crate::unity::*;

/// Version info grouped by minor version.
struct VersionInfoGroups<'a>(Vec<NonEmptyVec<VersionInfo<'a>>>);

/// Lists installed Unity versions.
pub(crate) fn list_versions(
    list_type: ListType,
    partial_version: Option<&str>,
) -> anyhow::Result<()> {
    let (dir, versions) = VersionList::from_installations()?;

    match list_type {
        ListType::Installed => {
            let installed = versions.prune(partial_version)?;
            println_b!(
                "Unity versions in: {} (*=default for new projects)",
                dir.display()
            );
            print_installed_versions(&installed);
            Ok(())
        }
        ListType::Updates => {
            let installed = versions.prune(partial_version)?;
            println_b!(
                "Updates for Unity versions in: {} (*=default for new projects)",
                dir.display(),
            );
            let ts = TermStat::new("Downloading", "release data...");
            let releases = fetch_unity_editor_releases()?;
            drop(ts);
            print_updates(&installed, &releases)
        }
        ListType::Latest => {
            // For this list type, it is ok to have no installed versions.
            let installed: Vec<_> = versions
                .prune(partial_version)
                .map(std::convert::Into::into)
                .unwrap_or_default();
            println_b!("Latest available minor releases");
            let ts = TermStat::new("Downloading", "release data...");
            let releases = fetch_unity_editor_releases()?;
            drop(ts);
            print_latest_versions(&installed, &releases, partial_version)
        }
        ListType::All => {
            // For this list type, it is ok to have no installed versions.
            let installed: Vec<_> = versions
                .prune(partial_version)
                .map(std::convert::Into::into)
                .unwrap_or_default();

            println_b!("Available releases");
            let ts = TermStat::new("Downloading", "release data...");
            let releases = fetch_unity_editor_releases()?;
            drop(ts);
            print_available_versions(&installed, &releases, partial_version)
        }
    }
}

/// Prints list of installed versions.
/// ```
/// Unity versions in: /Applications/Unity/Hub/Editor/ (*=default for new projects)
/// ┬─ 2022.3.51f1 - https://unity.com/releases/editor/whats-new/2022.3.51#notes
/// └─ 2022.3.52f1 * https://unity.com/releases/editor/whats-new/2022.3.52#notes
/// ┬─ 6000.0.24f1 - https://unity.com/releases/editor/whats-new/6000.0.24#notes
/// ├─ 6000.0.25f1 - https://unity.com/releases/editor/whats-new/6000.0.25#notes
/// └─ 6000.0.26f1 - https://unity.com/releases/editor/whats-new/6000.0.26#notes
/// ```
fn print_installed_versions(installed: &VersionList) {
    let default_version = installed.default_version();
    let version_groups = group_minor_versions(installed);
    let max_len = max_version_string_length(&version_groups);

    for group in version_groups.0 {
        for vi in group.iter() {
            print_list_marker(
                vi.version == group.first().version,
                vi.version == group.last().version,
            );

            let separator = if vi.version == default_version {
                '*'
            } else {
                '-'
            };
            let version_str = vi.version.to_string();

            println_b_if!(
                vi.version == default_version,
                " {:<max_len$} {} {}",
                version_str,
                separator,
                release_notes_url(vi.version).bright_blue()
            );
        }
    }
}

/// Prints list of installed versions and available updates.
/// ```
/// Updates for Unity versions in: /Applications/Unity/Hub/Editor/ (*=default for new projects)
/// ┬── LTS 2022.3.51f1
/// └── LTS 2022.3.52f1 * Up to date
/// ┬── LTS 6000.0.24f1
/// ├── LTS 6000.0.25f1 - Update(s) available
/// └── LTS 6000.0.26f1 - https://unity.com/releases/editor/whats-new/6000.0.26#notes > unityhub://6000.0.26f1/ccb7c73d2c02
/// ```
fn print_updates(installed: &VersionList, releases: &[ReleaseData]) -> anyhow::Result<()> {
    if releases.is_empty() {
        return Err(anyhow!("No update information available."));
    }

    let default_version = installed.default_version();
    let version_groups = collect_update_info(installed, releases);
    let max_version_len = max_version_string_length(&version_groups);

    for group in version_groups.0 {
        for vi in group.iter() {
            print_slim_list_marker(
                vi.version == group.first().version,
                vi.version == group.last().version,
            );

            let is_default = vi.version == default_version;
            let version_str = format!("{:<max_version_len$}", vi.version.to_string());
            let separator = if is_default { '*' } else { '-' };

            let rd = releases
                .iter()
                .find(|p| p.version == vi.version)
                .expect("Could not find release info for version");

            let (fill, stream) = fixed_stream_string(rd.stream);
            print!("{fill}");

            match &vi.v_type {
                VersionType::HasLaterInstalled => {
                    println_b_if!(is_default, "{} {}", stream, version_str);
                }
                VersionType::LatestInstalled => {
                    let last_in_group = vi.version == group.last().version;
                    if last_in_group {
                        println_b_if!(
                            is_default,
                            "{} {} {} Up to date",
                            stream.green(),
                            version_str.green(),
                            separator
                        );
                    } else {
                        println_b_if!(
                            is_default,
                            "{} {} {} Update(s) available",
                            stream.yellow(),
                            version_str.yellow(),
                            separator
                        );
                    };
                }
                VersionType::UpdateToLatest(release_info) => {
                    println_b_if!(
                        is_default,
                        "{} {} {} {} > {}",
                        stream,
                        version_str.blue(),
                        separator,
                        release_notes_url(release_info.version).bright_blue(),
                        release_info.unity_hub_deep_link.bright_blue()
                    );
                }
                VersionType::NoReleaseInfo => {
                    println_b_if!(
                        is_default,
                        "{} {} {} {}",
                        stream,
                        version_str,
                        separator,
                        format!("No {} update info available", vi.version.build_type,)
                            .bright_black()
                    );
                }
            };
        }
    }
    Ok(())
}

/// Groups installed versions by `major.minor` version
/// and collects update information for each installed version.
fn collect_update_info<'a>(
    installed: &'a VersionList,
    releases: &'a [ReleaseData],
) -> VersionInfoGroups<'a> {
    let mut version_groups = group_minor_versions(installed);

    // Add available updates to groups
    for group in &mut version_groups.0 {
        let latest_installed_version = group.last().version;

        let has_releases = releases.iter().any(|rd| {
            rd.version.major == latest_installed_version.major
                && rd.version.minor == latest_installed_version.minor
        });

        if has_releases {
            // Add update info to group (if there are any)
            releases
                .iter()
                .filter(|rd| {
                    rd.version.major == latest_installed_version.major
                        && rd.version.minor == latest_installed_version.minor
                        && rd.version > latest_installed_version
                })
                .for_each(|rd| {
                    group.push(VersionInfo {
                        version: rd.version,
                        v_type: VersionType::UpdateToLatest(rd),
                    });
                });
        } else {
            // No release info available for this minor version
            group.last_mut().v_type = VersionType::NoReleaseInfo;
        }
    }
    version_groups
}

/// Prints list of latest available Unity versions.
/// ```
/// ...
/// ┬─ TECH 2021.1.28f1 > unityhub://2021.1.28f1/f3f9dc10f3dd
/// ├─ TECH 2021.2.19f1 > unityhub://2021.2.19f1/602ecdbb2fb0
/// └── LTS 2021.3.45f1 > unityhub://2021.3.45f1/0da89fac8e79
/// ┬─ TECH 2022.1.24f1 > unityhub://2022.1.24f1/709dddfb713f
/// ├─ TECH 2022.2.21f1 > unityhub://2022.2.21f1/4907324dc95b
/// └── LTS 2022.3.52f1 - Installed: 2022.3.51f1, 2022.3.52f1
/// ┬─ TECH 2023.1.20f1 > unityhub://2023.1.20f1/35a524b12060
/// ├─ TECH 2023.2.20f1 > unityhub://2023.2.20f1/0e25a174756c
/// └─ BETA 2023.3.0b10 > unityhub://2023.3.0b10/52ddac442a2c
/// ┬── LTS 6000.0.26f1 - Installed: 6000.0.24f1, 6000.0.25f1 - update > unityhub://6000.0.26f1/ccb7c73d2c02
/// └ ALPHA 6000.1.0a3  > unityhub://6000.1.0a3/26ee3f072390
/// ...
/// ```
fn print_latest_versions(
    installed: &[Version],
    releases: &[ReleaseData],
    partial_version: Option<&str>,
) -> anyhow::Result<()> {
    // Get the latest version of each range.
    let minor_releases = latest_minor_releases(releases, partial_version);

    if minor_releases.is_empty() {
        return Err(anyhow!(
            "No releases available that match `{}`",
            partial_version.unwrap_or("*")
        ));
    }

    let max_len = minor_releases
        .iter()
        .map(|rd| rd.version.len())
        .max()
        .unwrap_or(0);

    let mut previous_major = None;
    let mut iter = minor_releases.iter().peekable();

    while let Some(latest) = iter.next() {
        let is_last_in_range = iter
            .peek()
            .map_or(true, |v| v.version.major != latest.version.major);

        print_slim_list_marker(
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
            let (fill, stream) = fixed_stream_string(latest.stream);
            print!("{fill}");
            let version = fixed_version_string(latest.version, max_len);

            println!(
                "{} {} > {}",
                stream,
                version,
                latest.unity_hub_deep_link.bright_blue(),
            );
        } else {
            print_installs_line(latest, &installed_in_range, max_len);
        }
    }
    Ok(())
}

fn fixed_stream_string(stream: ReleaseStream) -> (String, String) {
    let stream = stream.to_string();
    let line = "─".repeat(5 - stream.len());
    (format!("{} ", line), stream)
}

fn fixed_version_string(version: Version, max_len: usize) -> String {
    format!("{:<max_len$}", version.to_string())
}

/// Prints list of available Unity versions.
/// ```
/// Available releases
/// ...
/// ┬─ BETA 6000.0.0b11 - https://unity.com/releases/editor/beta/6000.0.0b11#notes > unityhub://6000.0.0b11/a707ca4efec4
/// ├─ BETA 6000.0.0b12 - https://unity.com/releases/editor/beta/6000.0.0b12#notes > unityhub://6000.0.0b12/0ac662189661
/// ├─ BETA 6000.0.0b13 - https://unity.com/releases/editor/beta/6000.0.0b13#notes > unityhub://6000.0.0b13/21aeb48b6ed2
/// ├─ BETA 6000.0.0b15 - https://unity.com/releases/editor/beta/6000.0.0b15#notes > unityhub://6000.0.0b15/8008bc0c1b74
/// ├─ BETA 6000.0.0b16 - https://unity.com/releases/editor/beta/6000.0.0b16#notes > unityhub://6000.0.0b16/1ddb887463a9
/// ├─ TECH 6000.0.0f1  - https://unity.com/releases/editor/whats-new/6000.0.0#notes > unityhub://6000.0.0f1/4ff56b3ea44c
/// ├─ TECH 6000.0.1f1  - https://unity.com/releases/editor/whats-new/6000.0.1#notes > unityhub://6000.0.1f1/d9cf669c6271
/// ├─ TECH 6000.0.2f1  - https://unity.com/releases/editor/whats-new/6000.0.2#notes > unityhub://6000.0.2f1/c36be92430b9
/// ├─ TECH 6000.0.3f1  - https://unity.com/releases/editor/whats-new/6000.0.3#notes > unityhub://6000.0.3f1/019aa96b6ed9
/// ├─ TECH 6000.0.4f1  - https://unity.com/releases/editor/whats-new/6000.0.4#notes > unityhub://6000.0.4f1/b5d5d06b038a
/// ...
/// ```
fn print_available_versions(
    installed: &[Version],
    releases: &[ReleaseData],
    partial_version: Option<&str>,
) -> anyhow::Result<()> {
    let releases = releases
        .iter()
        .filter(|r| partial_version.map_or(true, |p| r.version.to_string().starts_with(p)))
        .collect_vec();

    let Ok(versions) = VersionList::from_vec(releases.iter().map(|r| r.version).collect_vec())
    else {
        return Err(anyhow!(
            "No releases available that match `{}`",
            partial_version.unwrap_or("*")
        ));
    };

    let version_groups = group_minor_versions(&versions);
    let max_len = max_version_string_length(&version_groups);

    for group in version_groups.0 {
        for vi in group.iter() {
            print_slim_list_marker(
                vi.version == group.first().version,
                vi.version == group.last().version,
            );

            let is_installed = installed.contains(&vi.version);

            let rd = releases
                .iter()
                .find(|p| p.version == vi.version)
                .expect("Could not find release info for version");

            let version = fixed_version_string(rd.version, max_len);
            let (fill, stream) = fixed_stream_string(rd.stream);
            print!("{fill}");

            if is_installed {
                println_b!(
                    "{} {} - {} > installed",
                    stream.green(),
                    version.green(),
                    release_notes_url(vi.version).bright_blue()
                );
            } else {
                println!(
                    "{} {} - {} > {}",
                    stream,
                    version,
                    release_notes_url(vi.version).bright_blue(),
                    rd.unity_hub_deep_link.bright_blue()
                );
            }
        }
    }
    Ok(())
}

fn print_installs_line(latest: &ReleaseData, installed_in_range: &[Version], max_len: usize) {
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

    let (fill, stream) = fixed_stream_string(latest.stream);
    print!("{fill}");
    let version = fixed_version_string(latest.version, max_len);

    if is_up_to_date {
        println_b!(
            "{} {} - Installed: {}",
            stream.green(),
            version.green(),
            joined_versions
        );
    } else {
        println_b!(
            "{} {} - Installed: {} - update > {}",
            stream.yellow(),
            version.blue(),
            joined_versions,
            latest.unity_hub_deep_link.bright_blue()
        );
    };
}

struct VersionInfo<'a> {
    version: Version,
    v_type: VersionType<'a>,
}

enum VersionType<'a> {
    HasLaterInstalled,
    LatestInstalled,
    UpdateToLatest(&'a ReleaseData),
    NoReleaseInfo,
}

/// Returns the max length of the version strings ih the groups.
fn max_version_string_length(version_groups: &VersionInfoGroups<'_>) -> usize {
    version_groups
        .0
        .iter()
        .flat_map(|ne| ne.iter())
        .map(|vi| vi.version.len())
        .max()
        .unwrap_or(0)
}

/// Returns list of grouped versions that are in the same minor range.
fn group_minor_versions(installed: &VersionList) -> VersionInfoGroups<'_> {
    let mut version_groups = vec![];
    let mut group = vec![];

    for (i, &version) in installed.as_ref().iter().enumerate() {
        let next_version = installed.as_ref().get(i + 1);
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
            let v = NonEmptyVec::from_vec(std::mem::take(&mut group)).unwrap();
            version_groups.push(v);
        }
    }

    // Add the last group
    if let Ok(v) = NonEmptyVec::from_vec(group) {
        version_groups.push(v);
    }

    VersionInfoGroups(version_groups)
}

fn latest_minor_releases<'a>(
    releases: &'a [ReleaseData],
    partial_version: Option<&str>,
) -> Vec<&'a ReleaseData> {
    releases
        .iter()
        .filter(|r| partial_version.map_or(true, |p| r.version.to_string().starts_with(p)))
        .map(|r| (r.version.major, r.version.minor))
        .dedup()
        .filter_map(|(major, minor)| {
            releases
                .iter()
                .filter(|r| r.version.major == major && r.version.minor == minor)
                .max_by(|a, b| a.version.cmp(&b.version))
        })
        .collect()
}

/// Prints the list marker for the current item.
fn print_list_marker(is_first: bool, is_last: bool) {
    print!(
        "{}",
        match (is_first, is_last) {
            (true, true) => "──",
            (true, false) => "┬─",
            (false, false) => "├─",
            (false, true) => "└─",
        }
    );
}

/// Prints the slim list marker for the current item.
fn print_slim_list_marker(is_first: bool, is_last: bool) {
    print!(
        "{}",
        match (is_first, is_last) {
            (true, true) => "─",
            (true, false) => "┬",
            (false, false) => "├",
            (false, true) => "└",
        }
    );
}
