use anyhow::anyhow;
use itertools::Itertools;
use yansi::{Condition, Paint};

use crate::cli::ListType;
use crate::commands::*;
use crate::style_definitions::*;
use crate::unity::installations::{Installations, SortedVersions};
use crate::unity::release_api::{
    Releases, SortedReleases, UpdatePolicy, fetch_latest_releases, load_cached_releases,
};
use crate::unity::release_api_data::ReleaseData;
use crate::unity::{ReleaseStream, Version, release_notes_url};
use crate::utils::formatter::FormatWhen;
use crate::utils::report::{HeaderLevel, Report};
use crate::utils::vec1::Vec1;

//
// List command entry point
//

/// Lists installed Unity versions.
pub fn list_versions(
    list_type: ListType,
    version_prefix: Option<&str>,
    mode: UpdatePolicy,
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

//
// Installed versions
//

/// Prints list of installed versions.
/// ```
/// Unity versions in: /Applications/Unity/Hub/Editor/ (suggested: LTS 6000.0.36f1)
/// ── 2022.3.57f1 - https://unity.com/releases/editor/whats-new/2022.3.57#notes
/// ┬─ 6000.0.32f1 - https://unity.com/releases/editor/whats-new/6000.0.32#notes
/// ├─ 6000.0.35f1 - https://unity.com/releases/editor/whats-new/6000.0.35#notes
/// └─ 6000.0.36f1 * https://unity.com/releases/editor/whats-new/6000.0.36#notes
/// ```
fn display_installed_versions(installed: &Installations, mode: UpdatePolicy) -> anyhow::Result<()> {
    let releases = if mode == UpdatePolicy::Incremental {
        load_cached_releases()?
    } else {
        fetch_latest_releases(UpdatePolicy::ForceRefresh)?.into()
    };

    let report = Report::Terminal;
    report.header(
        format_args!(
            "Unity versions in: {di} {sv}",
            di = &installed
                .install_dir
                .normalized_display()
                .when(report.is_markdown())
                .md_code(),
            sv = format_suggested_version(&releases),
        ),
        HeaderLevel::H1,
    );

    if releases.is_empty() {
        display_basic_list(&installed.versions, &report);
    } else {
        display_list_with_release_dates(&installed.versions, &releases, &report);
    }

    Ok(())
}

fn display_basic_list(installed: &SortedVersions, report: &Report) {
    let version_groups = group_versions_by_minor(installed);
    let max_len = find_max_version_length(&version_groups);

    for group in version_groups.iter() {
        for info in group.iter() {
            let line_marker = wide_branch_marker(
                info.version == group.first().version,
                info.version == group.last().version,
            );

            report.paragraph(format_args!(
                "{line_marker} {vs:<max_len$} - {rn}",
                vs = info.version.to_interned_str(),
                rn = release_notes_url(info.version).paint(LINK)
            ));
        }
    }
}

fn display_list_with_release_dates(
    installed: &SortedVersions,
    releases: &Releases,
    report: &Report,
) {
    const CODE_BLOCK: &str = "```";
    let version_groups = group_versions_by_minor(installed);
    let max_len = find_max_version_length(&version_groups);

    report.when(report.is_markdown()).paragraph(CODE_BLOCK);

    for group in version_groups.iter() {
        for info in group.iter() {
            let is_suggested = Some(info.version) == releases.suggested_version;
            let release = releases.iter().find(|p| p.version == info.version);
            let stream = release.map_or(ReleaseStream::Other, |rd| rd.stream);

            let error_label = release.and_then(|rd| rd.error_label());
            let has_error = error_label.is_some();

            report.paragraph(format_args!(
                "{lm}{sp} {ri}",
                lm = slim_branch_marker(
                    info.version == group.first().version,
                    info.version == group.last().version,
                ),
                sp = stream_padding(stream),
                ri = format_args!(
                    "{stream} {vs} ({rd}) {mk} {description}",
                    vs = format_args!("{v:<max_len$}", v = info.version.to_interned_str())
                        .paint(ERROR)
                        .whenever(Condition::cached(has_error)),
                    rd = release.map_or_else(
                        || "----------".to_string(),
                        |rd| rd.release_date.format("%Y-%m-%d").to_string(),
                    ),
                    mk = if error_label.is_some() {
                        MARK_ERROR.paint(ERROR)
                    } else if is_suggested {
                        MARK_SUGGESTED.paint(UNSTYLED)
                    } else {
                        MARK_BULLET.paint(UNSTYLED)
                    },
                    description = format_release_description(info, release),
                )
                .bold()
                .whenever(Condition::cached(is_suggested))
            ));
        }
    }
    report.when(report.is_markdown()).paragraph(CODE_BLOCK);
}

//
// List updates
//

/// Prints list of installed versions and available updates.
/// ```
/// Updates for Unity versions in: /Applications/Unity/Hub/Editor/ (suggested: LTS 6000.0.36f1)
/// ─── LTS 2022.3.57f1 (2025-01-29) - Up to date
/// ┬── LTS 6000.0.32f1 (2024-12-19)
/// ├── LTS 6000.0.35f1 (2025-01-22) - Update(s) available
/// └── LTS 6000.0.36f1 (2025-01-28) * https://unity.com/releases/editor/whats-new/6000.0.36#notes
/// ```
fn display_updates(installed: &Installations, mode: UpdatePolicy) -> anyhow::Result<()> {
    let releases = fetch_latest_releases(mode)?;
    let report = Report::Terminal;
    report.header(
        format_args!(
            "Updates for Unity versions in: {di} {sv}",
            di = installed
                .install_dir
                .normalized_display()
                .when(report.is_markdown())
                .md_code(),
            sv = format_suggested_version(releases.as_ref())
        ),
        HeaderLevel::H1,
    );

    if releases.is_empty() {
        return Err(anyhow!("No update information is available."));
    }

    let version_groups = collect_version_update_info(&installed.versions, &releases);
    let max_version_len = find_max_version_length(&version_groups);

    for group in version_groups.iter() {
        for info in group.iter() {
            let is_suggested = Some(info.version) == releases.suggested_version();
            let version_str = format!("{:<max_version_len$}", info.version.to_interned_str());

            let release = releases.get_by_version(info.version)?;
            let release_date = release.release_date.format("%Y-%m-%d");

            let line_marker = slim_branch_marker(
                info.version == group.first().version,
                info.version == group.last().version,
            );
            let stream = release.stream;
            let stream_padding = stream_padding(stream);

            let error_label = release.error_label();
            let has_error = error_label.is_some();

            let release_info = match &info.version_type {
                VersionType::HasLaterInstalled => {
                    if let Some(el) = error_label {
                        format!(
                            "{stream} {vs} ({release_date}) {mk} [{el}]",
                            vs = version_str.paint(ERROR),
                            mk = MARK_ERROR.paint(ERROR),
                            el = format_label_with_url(el)
                        )
                    } else {
                        format!("{stream} {version_str} ({release_date})")
                    }
                }
                VersionType::LatestInstalled => {
                    let is_last_version_in_group = info.version == group.last().version;
                    let error_link = if let Some(el) = error_label {
                        format!(" [{el}]", el = format_label_with_url(el))
                    } else {
                        String::default()
                    };

                    if is_last_version_in_group {
                        let style = if has_error { ERROR } else { UP_TO_DATE };
                        format!(
                            "{stream} {vs} ({release_date}) {mk} Up to date{error_link}",
                            vs = version_str.paint(style),
                            mk = error_label
                                .map_or(MARK_UP_TO_DATE.paint(UNSTYLED), |_| MARK_ERROR
                                    .paint(ERROR)),
                        )
                    } else {
                        let style = if has_error { ERROR } else { HAS_UPDATE };
                        format!(
                            "{stream} {vs} ({release_date}) {mk} Update(s) available{error_link}",
                            vs = version_str.paint(style),
                            mk = error_label
                                .map_or(MARK_UPDATES_AVAILABLE.paint(UNSTYLED), |_| MARK_ERROR
                                    .paint(ERROR)),
                        )
                    }
                }
                VersionType::UpdateToLatest(release_info) => {
                    let style = if has_error { ERROR } else { IS_UPDATE };
                    format!(
                        "{stream} {vs} ({release_date}) {mk} {rd}",
                        vs = version_str.paint(style),
                        mk = error_label
                            .map_or(MARK_UPDATE_TO_LATEST.paint(UNSTYLED), |_| MARK_ERROR
                                .paint(ERROR)),
                        rd = format_release_description(info, Some(release_info)),
                    )
                }
                VersionType::NoReleaseInfo => {
                    format!(
                        "{stream} {version_str} ({release_date}) {MARK_NO_INFO} {}",
                        format_args!(
                            "No {bt} update info available",
                            bt = info.version.build_type,
                        )
                        .paint(NO_UPDATE_INFO)
                    )
                }
            }
            .clone();

            report.paragraph(format_args!(
                "{line_marker}{stream_padding} {ri}",
                ri = release_info
                    .bold()
                    .whenever(Condition::cached(is_suggested)),
            ));
        }
    }
    Ok(())
}

/// Groups installed versions by `major.minor` version
/// and collects update information for each installed version.
fn collect_version_update_info<'a>(
    installed: &'a SortedVersions,
    releases: &'a SortedReleases,
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
            // Add update info to the group (if there are any)
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

//
// List latest versions
//

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
    mode: UpdatePolicy,
) -> anyhow::Result<()> {
    let releases = fetch_latest_releases(mode)?;
    let report = Report::Terminal;
    report.header(
        format_args!(
            "Latest available minor releases {sv}",
            sv = format_suggested_version(releases.as_ref())
        ),
        HeaderLevel::H1,
    );

    // Get the latest version of each range.
    let minor_releases = collect_latest_minor_releases(&releases, version_prefix);

    if minor_releases.is_empty() {
        return Err(anyhow!(
            "No releases are available that match `{}`",
            version_prefix.unwrap_or("*")
        ));
    }

    let max_len = minor_releases
        .iter()
        .map(|rd| rd.version.to_interned_str().len())
        .fold(0, std::cmp::max);

    let mut previous_major = None;
    let mut iter = minor_releases.iter().peekable();

    while let Some(latest) = iter.next() {
        let is_last_in_range = iter
            .peek()
            .is_none_or(|v| v.version.major != latest.version.major);

        let line_marker = slim_branch_marker(
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
            let stream = latest.stream;

            report.paragraph(format_args!(
                "{line_marker}{sp} {stream} {vs} ({rd})",
                sp = stream_padding(stream),
                vs = format_version_with_padding(latest.version, max_len),
                rd = latest.release_date.format("%Y-%m-%d"),
            ));
        } else {
            display_installed_versions_line(
                &report,
                line_marker,
                latest,
                &installs_in_range,
                max_len,
            );
        }
    }
    Ok(())
}

fn format_suggested_version(releases: &Releases) -> String {
    releases
        .suggested_version
        .map_or_else(String::new, |suggested_version| {
            let stream = releases
                .iter()
                .find(|rd| rd.version == suggested_version)
                .map_or(ReleaseStream::Other, |rd| rd.stream);
            format!("(suggested: {stream} {suggested_version})")
        })
}

//
// List all versions
//

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
    mode: UpdatePolicy,
) -> anyhow::Result<()> {
    let mut releases = fetch_latest_releases(mode)?;
    let report = Report::Terminal;
    report.header(
        format_args!(
            "Available releases {sv}",
            sv = format_suggested_version(releases.as_ref())
        ),
        HeaderLevel::H1,
    );

    if let Some(prefix) = version_prefix {
        releases.retain(|rd| rd.version.to_interned_str().starts_with(prefix));
    }

    let Ok(versions) = SortedVersions::try_from(releases.iter().map(|rd| rd.version).collect_vec())
    else {
        return Err(anyhow!(
            "No releases are available that match `{}`",
            version_prefix.unwrap_or("*")
        ));
    };

    let version_groups = group_versions_by_minor(&versions);
    let max_len = find_max_version_length(&version_groups);

    for group in version_groups.iter() {
        for info in group.iter() {
            let line_marker = slim_branch_marker(
                info.version == group.first().version,
                info.version == group.last().version,
            );

            let is_installed = installed.is_some_and(|i| i.versions.contains(&info.version));
            let release = releases.get_by_version(info.version)?;
            let release_date = release.release_date.format("%Y-%m-%d");
            let version = format_version_with_padding(release.version, max_len);
            let stream = release.stream;

            let error_label = release.error_label();
            let has_error = error_label.is_some();

            let description = format_release_description(info, Some(release));
            let mark = if error_label.is_some() {
                MARK_ERROR.paint(ERROR)
            } else {
                MARK_BULLET.paint(UNSTYLED)
            };

            report.paragraph(format_args!(
                "{line_marker}{sp} {ds}",
                sp = stream_padding(stream),
                ds = if is_installed {
                    let style = if has_error { ERROR } else { UP_TO_DATE };
                    format!(
                        "{stream} {vs} ({release_date}) {mark} {description} > installed",
                        vs = version.paint(style),
                    )
                } else {
                    let style = if has_error { ERROR } else { UNSTYLED };
                    format!(
                        "{stream} {vs} ({release_date}) {mark} {description}",
                        vs = version.paint(style)
                    )
                }
                .bold()
                .whenever(Condition::cached(is_installed))
            ));
        }
    }
    Ok(())
}

fn display_installed_versions_line(
    report: &Report,
    line_marker: &str,
    latest: &ReleaseData,
    installed_in_range: &[Version],
    max_len: usize,
) {
    let is_up_to_date = installed_in_range
        .last()
        .filter(|&v| v == &latest.version)
        .is_some()
        || installed_in_range // Special case for when an installed version is newer than the latest.
            .last()
            .is_some_and(|&v| v > latest.version);

    let stream = latest.stream;
    let version = format_version_with_padding(latest.version, max_len);
    let release_date = latest.release_date.format("%Y-%m-%d");
    let joined_versions = installed_in_range.iter().join(", ");

    report.paragraph(format_args!(
        "{line_marker}{sp} {ri}",
        sp = stream_padding(stream),
        ri = if is_up_to_date {
            format!(
                "{stream} {vs} ({release_date}) {MARK_UP_TO_DATE} Installed: {joined_versions}",
                vs = version.paint(UP_TO_DATE),
            ).bold().to_string()
        } else {
            format!(
                "{stream} {vs} ({release_date}) {MARK_UPDATES_AVAILABLE} Installed: {joined_versions} - update available",
                vs = version.paint(IS_UPDATE),
            ).bold().to_string()
        }
    ));
}

//
// Helpers
//

/// Version info grouped by minor version.
struct VersionInfoGroups<'a>(Vec<Vec1<VersionInfo<'a>>>);

impl<'a> VersionInfoGroups<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &Vec1<VersionInfo<'a>>> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Vec1<VersionInfo<'a>>> {
        self.0.iter_mut()
    }
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

const fn stream_padding(stream: ReleaseStream) -> &'static str {
    match stream {
        ReleaseStream::Beta | ReleaseStream::Tech | ReleaseStream::Supp => "─",
        ReleaseStream::Lts => "──",
        ReleaseStream::Other | ReleaseStream::Alpha => "",
    }
}

fn format_version_with_padding(version: Version, max_len: usize) -> String {
    format!("{v:<max_len$}", v = version.to_interned_str())
}

/// Returns the max length of the version strings in the groups.
fn find_max_version_length(version_groups: &VersionInfoGroups<'_>) -> usize {
    version_groups
        .iter()
        .flat_map(|v| v.iter())
        .map(|vi| vi.version.to_interned_str().len())
        .fold(0, std::cmp::max)
}

/// Returns a list of grouped versions that are in the same minor range.
fn group_versions_by_minor(installed: &SortedVersions) -> VersionInfoGroups<'_> {
    let version_groups = installed
        .iter()
        .chunk_by(|v| (v.major, v.minor))
        .into_iter()
        .filter_map(|(_, group)| create_version_info_group(group))
        .collect();

    VersionInfoGroups(version_groups)
}

/// Creates a version info group from the given versions.
/// Returns `None` if the group is empty.
fn create_version_info_group<'a, I>(versions: I) -> Option<Vec1<VersionInfo<'a>>>
where
    I: Iterator<Item = &'a Version>,
{
    let mut peekable = versions.peekable();
    let mut infos = Vec::new();

    while let Some(&version) = peekable.next() {
        let version_type = if peekable.peek().is_none() {
            VersionType::LatestInstalled
        } else {
            VersionType::HasLaterInstalled
        };

        infos.push(VersionInfo {
            version,
            version_type,
        });
    }

    Vec1::try_from(infos).ok()
}

fn format_release_description(info: &VersionInfo, release: Option<&ReleaseData>) -> String {
    if let Some(s) = release.and_then(|rd| rd.error_label().map(format_label_with_url)) {
        format!("{} [{}]", release_notes_url(info.version).paint(LINK), s)
    } else {
        release_notes_url(info.version).paint(LINK).to_string()
    }
}

fn collect_latest_minor_releases<'a>(
    releases: &'a SortedReleases,
    version_prefix: Option<&str>,
) -> Vec<&'a ReleaseData> {
    releases
        .iter()
        .filter(|rd| version_prefix.is_none_or(|p| rd.version.to_interned_str().starts_with(p)))
        .chunk_by(|rd| (rd.version.major, rd.version.minor))
        .into_iter()
        .filter_map(|(_, group)| group.last()) // Get the latest version of each range.
        .collect()
}

fn wide_branch_marker(is_first: bool, is_last: bool) -> &'static str {
    match (is_first, is_last) {
        (true, true) => "──",
        (true, false) => "┬─",
        (false, false) => "├─",
        (false, true) => "└─",
    }
}

fn slim_branch_marker(is_first: bool, is_last: bool) -> &'static str {
    match (is_first, is_last) {
        (true, true) => "─",
        (true, false) => "┬",
        (false, false) => "├",
        (false, true) => "└",
    }
}
