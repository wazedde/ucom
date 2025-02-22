use anyhow::anyhow;
use crossterm::style::Stylize;
use itertools::Itertools;
use std::ops::{Deref, DerefMut};
use yansi::Paint;

use crate::cli::ListType;
use crate::commands::{println_bold, println_conditional_bold};
use crate::unity::installations::{Installations, VersionList};
use crate::unity::release_api::{
    Mode, ReleaseCollection, SortedReleaseCollection, fetch_latest_releases, load_cached_releases,
};
use crate::unity::release_api_data::ReleaseData;
use crate::unity::{ReleaseStream, Version, release_notes_url};
use crate::utils::vec1::Vec1;

/// Version info grouped by minor version.
struct VersionInfoGroups<'a>(Vec<Vec1<VersionInfo<'a>>>);

impl<'a> Deref for VersionInfoGroups<'a> {
    type Target = Vec<Vec1<VersionInfo<'a>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for VersionInfoGroups<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Lists installed Unity versions.
pub fn list_versions(
    list_type: ListType,
    version_prefix: Option<&str>,
    mode: Mode,
) -> anyhow::Result<()> {
    match list_type {
        ListType::Installed => {
            let installed = Installations::find_installations(version_prefix)?;
            display_installed_versions(&installed, mode)
        }
        ListType::Updates => {
            let installed = Installations::find_installations(version_prefix)?;
            display_updates(&installed, mode)
        }
        ListType::Latest => {
            let installed = Installations::try_find_installations(version_prefix);
            display_latest_versions(installed.as_ref(), version_prefix, mode)
        }
        ListType::All => {
            let installed = Installations::try_find_installations(version_prefix);
            display_available_versions(installed.as_ref(), version_prefix, mode)
        }
    }
}

/// Prints list of installed versions.
/// ```
/// Unity versions in: /Applications/Unity/Hub/Editor/ (suggested: LTS 6000.0.36f1)
/// ── 2022.3.57f1 - https://unity.com/releases/editor/whats-new/2022.3.57#notes
/// ┬─ 6000.0.32f1 - https://unity.com/releases/editor/whats-new/6000.0.32#notes
/// ├─ 6000.0.35f1 - https://unity.com/releases/editor/whats-new/6000.0.35#notes
/// └─ 6000.0.36f1 * https://unity.com/releases/editor/whats-new/6000.0.36#notes
/// ```
fn display_installed_versions(installed: &Installations, mode: Mode) -> anyhow::Result<()> {
    let releases = if mode == Mode::Auto {
        load_cached_releases()?
    } else {
        fetch_latest_releases(Mode::Force)?.into()
    };

    println_bold!(
        "Unity versions in: {} {}",
        installed.install_dir.display(),
        format_suggested_version(&releases)
    );

    if releases.is_empty() {
        display_basic_list(&installed.versions);
    } else {
        display_list_with_release_dates(&installed.versions, &releases);
    }

    Ok(())
}

fn display_basic_list(installed: &VersionList) {
    let version_groups = group_versions_by_minor(installed);
    let max_len = find_max_version_length(&version_groups);

    for group in version_groups.iter() {
        for info in group.iter() {
            print_list_marker(
                info.version == group.first().version,
                info.version == group.last().version,
            );
            let separator = '-';
            let version_str = info.version.as_str();

            println!(
                " {:<max_len$} {} {}",
                version_str,
                separator,
                release_notes_url(info.version).bright_blue()
            );
        }
    }
}

fn display_list_with_release_dates(installed: &VersionList, releases: &ReleaseCollection) {
    let version_groups = group_versions_by_minor(installed);
    let max_len = find_max_version_length(&version_groups);

    for group in version_groups.iter() {
        for info in group.iter() {
            print_slim_list_marker(
                info.version == group.first().version,
                info.version == group.last().version,
            );

            let is_suggested = Some(info.version) == releases.suggested_version;
            let version_str = format!("{:<max_len$}", info.version.as_str());
            let separator = if is_suggested { '*' } else { '-' };

            let rd = releases.iter().find(|p| p.version == info.version);

            let release_date = rd.map_or("----------".to_string(), |rd| {
                rd.release_date.format("%Y-%m-%d").to_string()
            });

            let (padding, stream) =
                format_release_stream_with_padding(rd.map_or(ReleaseStream::Other, |rd| rd.stream));
            print!("{padding}");

            println_conditional_bold!(
                is_suggested,
                "{} {} ({}) {} {}",
                stream,
                version_str,
                release_date,
                separator,
                release_notes_url(info.version).bright_blue()
            );
        }
    }
}

/// Prints list of installed versions and available updates.
/// ```
/// Updates for Unity versions in: /Applications/Unity/Hub/Editor/ (suggested: LTS 6000.0.36f1)
/// ─── LTS 2022.3.57f1 (2025-01-29) - Up to date
/// ┬── LTS 6000.0.32f1 (2024-12-19)
/// ├── LTS 6000.0.35f1 (2025-01-22) - Update(s) available
/// └── LTS 6000.0.36f1 (2025-01-28) * https://unity.com/releases/editor/whats-new/6000.0.36#notes
/// ```
fn display_updates(installed: &Installations, mode: Mode) -> anyhow::Result<()> {
    let releases = fetch_latest_releases(mode)?;
    println_bold!(
        "Updates for Unity versions in: {} {}",
        installed.install_dir.display(),
        format_suggested_version(&releases)
    );

    if releases.is_empty() {
        return Err(anyhow!("No update information available."));
    }

    let version_groups = collect_version_update_info(&installed.versions, &releases);
    let max_version_len = find_max_version_length(&version_groups);

    for group in version_groups.iter() {
        for info in group.iter() {
            print_slim_list_marker(
                info.version == group.first().version,
                info.version == group.last().version,
            );

            let is_suggested = Some(info.version) == releases.suggested_version;
            let version_str = format!("{:<max_version_len$}", info.version.as_str());
            let separator = if is_suggested { '*' } else { '-' };

            let rd = releases
                .iter()
                .find(|p| p.version == info.version)
                .expect("Guaranteed to exist");

            let release_date = rd.release_date.format("%Y-%m-%d");

            let (padding, stream) = format_release_stream_with_padding(rd.stream);
            print!("{padding}");

            match &info.version_type {
                VersionType::HasLaterInstalled => {
                    println_conditional_bold!(
                        is_suggested,
                        "{} {} ({})",
                        stream,
                        version_str,
                        release_date
                    );
                }
                VersionType::LatestInstalled => {
                    let last_in_group = info.version == group.last().version;
                    if last_in_group {
                        println_conditional_bold!(
                            is_suggested,
                            "{} {} ({}) {} Up to date",
                            stream.green(),
                            version_str.green(),
                            release_date,
                            separator
                        );
                    } else {
                        println_conditional_bold!(
                            is_suggested,
                            "{} {} ({}) {} Update(s) available",
                            stream.yellow(),
                            version_str.yellow(),
                            release_date,
                            separator
                        );
                    };
                }
                VersionType::UpdateToLatest(release_info) => {
                    println_conditional_bold!(
                        is_suggested,
                        "{} {} ({}) {} {}",
                        stream.blue(),
                        version_str.blue(),
                        release_date,
                        separator,
                        release_notes_url(release_info.version).bright_blue()
                    );
                }
                VersionType::NoReleaseInfo => {
                    println_conditional_bold!(
                        is_suggested,
                        "{} {} ({}) {} {}",
                        stream,
                        version_str,
                        release_date,
                        separator,
                        format!("No {} update info available", info.version.build_type,)
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
fn collect_version_update_info<'a>(
    installed: &'a VersionList,
    releases: &'a SortedReleaseCollection,
) -> VersionInfoGroups<'a> {
    let mut version_groups = group_versions_by_minor(installed);

    // Add available updates to groups
    for group in version_groups.iter_mut() {
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
                        version_type: VersionType::UpdateToLatest(rd),
                    });
                });
        } else {
            // No release info available for this minor version
            group.last_mut().version_type = VersionType::NoReleaseInfo;
        }
    }
    version_groups
}

/// Prints list of latest available Unity versions.
/// ```
/// ...
/// ┬─ TECH 2022.1.24f1 (2022-12-06)
/// ├─ TECH 2022.2.21f1 (2023-05-24)
/// └── LTS 2022.3.57f1 (2025-01-29) - Installed: 2022.3.57f1
/// ┬─ TECH 2023.1.20f1 (2023-11-09)
/// ├─ TECH 2023.2.20f1 (2024-04-25)
/// └─ BETA 2023.3.0b10 (2024-03-05)
/// ┬── LTS 6000.0.36f1 (2025-01-28) - Installed: 6000.0.32f1, 6000.0.35f1 - update available
/// ├─ BETA 6000.1.0b4  (2025-01-28)
/// └ ALPHA 6000.2.0a1  (2025-01-29)
/// ...
/// ```
fn display_latest_versions(
    installed: Option<&Installations>,
    version_prefix: Option<&str>,
    mode: Mode,
) -> anyhow::Result<()> {
    let releases = fetch_latest_releases(mode)?;
    println_bold!(
        "Latest available minor releases {}",
        format_suggested_version(&releases)
    );

    // Get the latest version of each range.
    let minor_releases = collect_latest_minor_releases(&releases, version_prefix);

    if minor_releases.is_empty() {
        return Err(anyhow!(
            "No releases available that match `{}`",
            version_prefix.unwrap_or("*")
        ));
    }

    let max_len = minor_releases
        .iter()
        .map(|rd| rd.version.as_str().len())
        .fold(0, std::cmp::max);

    let mut previous_major = None;
    let mut iter = minor_releases.iter().peekable();

    while let Some(latest) = iter.next() {
        let is_last_in_range = iter
            .peek()
            .is_none_or(|v| v.version.major != latest.version.major);

        print_slim_list_marker(
            Some(latest.version.major) != previous_major,
            is_last_in_range,
        );

        previous_major = Some(latest.version.major);

        // Find all installed versions in the same range as the latest version.
        let installs_in_range = installed
            .map(|i| {
                i.versions
                    .iter()
                    .filter(|v| v.major == latest.version.major && v.minor == latest.version.minor)
                    .copied()
                    .collect_vec()
            })
            .unwrap_or_default();

        if installs_in_range.is_empty() {
            // No installed versions in the range.
            let (padding, stream) = format_release_stream_with_padding(latest.stream);
            print!("{padding}");
            let version = format_version_with_padding(latest.version, max_len);
            let release_date = latest.release_date.format("%Y-%m-%d");

            println!("{stream} {version} ({release_date})",);
        } else {
            display_installed_versions_line(latest, &installs_in_range, max_len);
        }
    }
    Ok(())
}

fn format_suggested_version(releases: &ReleaseCollection) -> String {
    releases
        .suggested_version
        .map_or_else(String::new, |suggested_version| {
            let stream = releases
                .iter()
                .find(|x| x.version == suggested_version)
                .map_or(ReleaseStream::Other, |x| x.stream);
            format!("(suggested: {stream} {suggested_version})")
        })
}

fn format_release_stream_with_padding(stream: ReleaseStream) -> (String, String) {
    let stream = stream.to_string();
    let padding = "─".repeat(5 - stream.len());
    (format!("{padding} "), stream)
}

fn format_version_with_padding(version: Version, max_len: usize) -> String {
    format!("{:<max_len$}", version.as_str())
}

/// Prints list of available Unity versions.
/// ```
/// Available releases
/// ...
/// ┬─ BETA 6000.0.0b11 (2024-03-13) - https://unity.com/releases/editor/beta/6000.0.0b11#notes
/// ├─ BETA 6000.0.0b12 (2024-03-19) - https://unity.com/releases/editor/beta/6000.0.0b12#notes
/// ├─ BETA 6000.0.0b13 (2024-03-27) - https://unity.com/releases/editor/beta/6000.0.0b13#notes
/// ├─ BETA 6000.0.0b15 (2024-04-13) - https://unity.com/releases/editor/beta/6000.0.0b15#notes
/// ├─ BETA 6000.0.0b16 (2024-04-19) - https://unity.com/releases/editor/beta/6000.0.0b16#notes
/// ├─ TECH 6000.0.0f1  (2024-04-29) - https://unity.com/releases/editor/whats-new/6000.0.0#notes
/// ├─ TECH 6000.0.1f1  (2024-05-08) - https://unity.com/releases/editor/whats-new/6000.0.1#notes
/// ├─ TECH 6000.0.2f1  (2024-05-14) - https://unity.com/releases/editor/whats-new/6000.0.2#notes
/// ...
/// ```
fn display_available_versions(
    installed: Option<&Installations>,
    version_prefix: Option<&str>,
    mode: Mode,
) -> anyhow::Result<()> {
    let releases = fetch_latest_releases(mode)?;
    println_bold!("Available releases {}", format_suggested_version(&releases));

    let releases = releases
        .iter()
        .filter(|r| version_prefix.is_none_or(|p| r.version.as_str().starts_with(p)))
        .collect_vec();

    let Ok(versions) = VersionList::try_from(releases.iter().map(|r| r.version).collect_vec())
    else {
        return Err(anyhow!(
            "No releases available that match `{}`",
            version_prefix.unwrap_or("*")
        ));
    };

    let version_groups = group_versions_by_minor(&versions);
    let max_len = find_max_version_length(&version_groups);

    for group in version_groups.iter() {
        for info in group.iter() {
            print_slim_list_marker(
                info.version == group.first().version,
                info.version == group.last().version,
            );

            let is_installed = installed.is_some_and(|i| i.versions.contains(&info.version));

            let release = releases
                .iter()
                .find(|p| p.version == info.version)
                .expect("Guaranteed to exist");

            let release_date = release.release_date.format("%Y-%m-%d");

            let version = format_version_with_padding(release.version, max_len);
            let (padding, stream) = format_release_stream_with_padding(release.stream);
            print!("{padding}");

            if is_installed {
                println_bold!(
                    "{} {} ({}) - {} > installed",
                    stream.green(),
                    version.green(),
                    release_date,
                    release_notes_url(info.version).bright_blue()
                );
            } else {
                println!(
                    "{} {} ({}) - {}",
                    stream,
                    version,
                    release_date,
                    release_notes_url(info.version).bright_blue()
                );
            }
        }
    }
    Ok(())
}

fn display_installed_versions_line(
    latest: &ReleaseData,
    installed_in_range: &[Version],
    max_len: usize,
) {
    let is_up_to_date = installed_in_range
        .last()
        .filter(|&v| v == &latest.version)
        .is_some()
        || installed_in_range // Special case for when installed version is newer than the latest.
            .last()
            .is_some_and(|&v| v > latest.version);

    let joined_versions = installed_in_range.iter().join(", ");

    let (padding, stream) = format_release_stream_with_padding(latest.stream);
    print!("{padding}");
    let version = format_version_with_padding(latest.version, max_len);
    let release_date = latest.release_date.format("%Y-%m-%d");

    if is_up_to_date {
        println_bold!(
            "{} {} ({}) - Installed: {}",
            stream.green(),
            version.green(),
            release_date,
            joined_versions
        );
    } else {
        println_bold!(
            "{} {} ({}) - Installed: {} - update available",
            stream.blue(),
            version.blue(),
            release_date,
            joined_versions
        );
    };
}

struct VersionInfo<'a> {
    version: Version,
    version_type: VersionType<'a>,
}

enum VersionType<'a> {
    HasLaterInstalled,
    LatestInstalled,
    UpdateToLatest(&'a ReleaseData),
    NoReleaseInfo,
}

/// Returns the max length of the version strings in the groups.
fn find_max_version_length(version_groups: &VersionInfoGroups<'_>) -> usize {
    version_groups
        .iter()
        .flat_map(|v| v.iter())
        .map(|vi| vi.version.as_str().len())
        .fold(0, std::cmp::max)
}

/// Returns list of grouped versions that are in the same minor range.
fn group_versions_by_minor(installed: &VersionList) -> VersionInfoGroups<'_> {
    let version_groups = installed
        .iter()
        .chunk_by(|v| (v.major, v.minor))
        .into_iter()
        .filter_map(|(_, group)| build_version_info_group(group.collect_vec()))
        .collect();

    VersionInfoGroups(version_groups)
}

fn build_version_info_group(versions: Vec<&Version>) -> Option<Vec1<VersionInfo<'_>>> {
    let len = versions.len();

    let infos = versions
        .into_iter()
        .enumerate()
        .map(|(i, &version)| {
            let version_type = if i == len - 1 {
                VersionType::LatestInstalled
            } else {
                VersionType::HasLaterInstalled
            };
            VersionInfo {
                version,
                version_type,
            }
        })
        .collect_vec();

    Vec1::try_from(infos).ok()
}

fn collect_latest_minor_releases<'a>(
    releases: &'a SortedReleaseCollection,
    version_prefix: Option<&str>,
) -> Vec<&'a ReleaseData> {
    releases
        .iter()
        .filter(|r| version_prefix.is_none_or(|p| r.version.as_str().starts_with(p)))
        .chunk_by(|r| (r.version.major, r.version.minor))
        .into_iter()
        .filter_map(|(_, group)| group.last()) // Get the latest version of each range.
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
