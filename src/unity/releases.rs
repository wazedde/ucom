use crate::unity::http_cache;
use crate::unity::{BuildType, Major, Minor, Version};
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use itertools::Itertools;
use regex::Regex;
use select::document::Document;
use select::predicate::{Class, Name};
use serde::de::{self, Deserializer};
use serde::Deserialize;
const RELEASES_ARCHIVE_URL: &str = "https://unity.com/releases/editor/archive";

#[derive(Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub(crate) struct ReleaseInfo {
    #[serde(deserialize_with = "deserialize_version")]
    pub(crate) version: Version,
    #[serde(rename = "releaseDate")]
    // pub(crate) release_date: String,
    pub(crate) release_date: DateTime<Utc>,
    #[serde(rename = "unityHubDeepLink")]
    pub(crate) installation_url: String,
    pub(crate) stream: String,
}

#[allow(dead_code)]
pub(crate) enum ReleaseFilter {
    /// Match all releases.
    All,
    /// Match releases on the major version.
    Major { major: Major },
    /// Match releases on the major and minor version.
    Minor { major: Major, minor: Minor },
}

impl ReleaseFilter {
    /// Returns true if the version matches the filter.
    const fn eval(&self, v: Version) -> bool {
        match self {
            Self::All => true,
            Self::Major { major } => v.major == *major,
            Self::Minor { major, minor } => v.major == *major && v.minor == *minor,
        }
    }
}

/// Gets Unity releases from the Unity website.
pub(crate) fn fetch_unity_editor_releases() -> anyhow::Result<Vec<ReleaseInfo>> {
    let body = http_cache::fetch_content(RELEASES_ARCHIVE_URL, true)?;
    let releases = extract_releases_from_html(&body, &ReleaseFilter::All);
    Ok(releases)
}

/// Gets the current and update releases for the given version from the Unity website.
pub(crate) fn fetch_update_info(
    version: Version,
) -> anyhow::Result<(Option<ReleaseInfo>, Vec<ReleaseInfo>)> {
    let body = http_cache::fetch_content(RELEASES_ARCHIVE_URL, true)?;
    let releases = extract_releases_from_html(
        &body,
        &ReleaseFilter::Minor {
            major: version.major,
            minor: version.minor,
        },
    );

    let current = releases.iter().find(|ri| ri.version == version).cloned();
    let updates = releases
        .into_iter()
        .filter(|ri| ri.version > version) // Only newer versions.
        .collect();

    Ok((current, updates))
}

pub(crate) type Url = String;

/// Gets the release notes for the given version from the Unity website.
pub(crate) fn fetch_release_notes(version: Version) -> anyhow::Result<(Url, String)> {
    let url = release_notes_url(version);
    let body = http_cache::fetch_content(&url, true)?;
    Ok((url, body))
}

fn deserialize_version<'de, D>(deserializer: D) -> Result<Version, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    s.parse::<Version>()
        .map_err(|_| de::Error::custom(format!("Invalid version format: {}", s)))
}

/// Extracts releases from the html that match the filter.
fn extract_releases_from_html(html: &str, filter: &ReleaseFilter) -> Vec<ReleaseInfo> {
    let prefix = "self.__next_f.push([1,\"";
    let suffix = "\"])";

    let doc = Document::from(html);
    let data = doc
        .find(Name("script"))
        .filter_map(|script| {
            script
                .text()
                .strip_prefix(prefix)
                .and_then(|t| t.strip_suffix(suffix))
                .map(|t| t.replace("\\n", "\n").replace("\\\"", "\""))
        })
        .collect::<String>();

    let re = Regex::new("^[0-9A-Fa-f]+:(.*)").unwrap();
    let mut releases = data
        .lines()
        .filter_map(|line| {
            re.captures(line)
                .and_then(|caps| caps.get(1))
                .map(|m| m.as_str())
                .filter(|stripped_line| stripped_line.starts_with("{\"version\":"))
                .and_then(|stripped_line| serde_json::from_str::<ReleaseInfo>(stripped_line).ok())
                .filter(|version_info| filter.eval(version_info.version))
        })
        .collect_vec();

    releases.sort_unstable();
    releases
}

/// Get the version from the url.
/// The url looks like: `unityhub://2021.2.14f1/bcb93e5482d2`
#[allow(dead_code)]
fn version_from_url(url: &str) -> Option<Version> {
    let version_part = url.split('/').rev().nth(1)?;
    version_part.parse::<Version>().ok()
}

pub(crate) fn release_notes_url(version: Version) -> Url {
    match version.build_type {
        BuildType::Alpha => format!("https://unity.com/releases/editor/alpha/{version}#notes"),
        BuildType::Beta => format!("https://unity.com/releases/editor/beta/{version}#notes"),
        BuildType::Final | BuildType::ReleaseCandidate => {
            let version = format!("{}.{}.{}", version.major, version.minor, version.patch);
            format!("https://unity.com/releases/editor/whats-new/{version}#notes")
        }
        BuildType::FinalPatch => {
            format!("https://unity.com/releases/editor/whats-new/{version}#notes")
        }
    }
}

/// Extracts release notes from the supplied html.
pub(crate) fn extract_release_notes(html: &str) -> IndexMap<String, Vec<String>> {
    let document = Document::from(html);
    let mut release_notes = IndexMap::<String, Vec<String>>::new();

    if let Some(node) = document.find(Class("release-notes")).next() {
        let mut topic_header = "General".to_string();

        // Iterate over the children of the release notes node.
        node.children().for_each(|n| match n.name() {
            // The topic header is the h3 or h4 node.
            Some("h3" | "h4") => topic_header = n.text(),
            // The topic list is the ul node.
            Some("ul") => {
                // Iterate over the list items and add them to the topic list.
                let topic_list = release_notes.entry(topic_header.clone()).or_default();
                n.find(Name("li")).for_each(|li| {
                    if let Some(release_note_line) = li.text().lines().next() {
                        topic_list.push(release_note_line.to_string());
                    }
                });
            }
            _ => (),
        });
    }

    release_notes
}

#[cfg(test)]
mod releases_tests {
    use std::str::FromStr;

    use crate::unity::{ReleaseFilter, Version};

    use super::{extract_releases_from_html, version_from_url};

    #[test]
    fn test_version_from_url() {
        let url = "unityhub://2021.2.14f1/bcb93e5482d2";
        let version = version_from_url(url).unwrap();
        assert_eq!(version, Version::from_str("2021.2.14f1").unwrap());
    }

    #[test]
    fn test_version_from_url_invalid_url() {
        let url = "unityhub://2021.2.14f1";
        let version = version_from_url(url);
        assert!(version.is_none());
    }

    #[test]
    fn test_version_from_url_invalid_version() {
        let url = "unityhub://2021.2.14/bcb93e5482d2";
        let version = version_from_url(url);
        assert!(version.is_none());
    }

    #[test]
    fn test_find_releases_all() {
        let html = include_str!("test_data/unity_download_archive.html");
        let releases = extract_releases_from_html(html, &ReleaseFilter::All);
        assert_eq!(releases.len(), 810);
    }

    #[test]
    fn test_find_releases_major() {
        let html = include_str!("test_data/unity_download_archive.html");
        let releases = extract_releases_from_html(html, &ReleaseFilter::Major { major: 2021 });
        assert_eq!(releases.len(), 91);
    }

    #[test]
    fn test_find_releases_minor() {
        let html = include_str!("test_data/unity_download_archive.html");
        let releases = extract_releases_from_html(
            html,
            &ReleaseFilter::Minor {
                major: 2019,
                minor: 2,
            },
        );
        assert_eq!(releases.len(), 22);
    }

    #[test]
    fn test_release_notes_url() {
        let version = Version::from_str("2021.2.14f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(url, "https://unity.com/releases/editor/whats-new/2021.2.14");
    }

    #[test]
    fn test_release_notes_url_5_1_0_1() {
        let version = Version::from_str("5.1.0f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(url, "https://unity.com/releases/editor/whats-new/5.1.0");
    }

    #[test]
    fn test_release_notes_url_5_1_0_2() {
        let version = Version::from_str("5.1.0f2").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(url, "https://unity.com/releases/editor/whats-new/5.1.0-0");
    }

    #[test]
    fn test_release_notes_url_5_1_0_3() {
        let version = Version::from_str("5.1.0f3").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(url, "https://unity.com/releases/editor/whats-new/5.1.0-1");
    }

    #[test]
    fn test_release_notes_5_0_0() {
        let html = include_str!("test_data/unity_5_0_0.html");
        let release_notes = super::extract_release_notes(html);
        assert_eq!(release_notes.len(), 47);
        assert_eq!(release_notes.values().flatten().count(), 1114);
    }

    #[test]
    fn test_release_notes_2017_1_0() {
        let html = include_str!("test_data/unity_2017_1_0.html");
        let release_notes = super::extract_release_notes(html);
        assert_eq!(release_notes.len(), 6);
        assert_eq!(release_notes.values().flatten().count(), 440);
    }

    #[test]
    fn test_release_notes_2017_2_5() {
        let html = include_str!("test_data/unity_2017_2_5.html");
        let release_notes = super::extract_release_notes(html);
        assert_eq!(release_notes.len(), 1);
        assert_eq!(release_notes.values().flatten().count(), 10);
    }

    #[test]
    fn test_release_notes_2021_3_17() {
        let html = include_str!("test_data/unity_2021_3_17.html");
        let release_notes = super::extract_release_notes(html);
        assert_eq!(release_notes.len(), 7);
        assert_eq!(release_notes.values().flatten().count(), 204);
    }

    #[test]
    fn test_release_notes_2022_2_0() {
        let html = include_str!("test_data/unity_2022_2_0.html");
        let release_notes = super::extract_release_notes(html);
        assert_eq!(release_notes.len(), 7);
        assert_eq!(release_notes.values().flatten().count(), 2090);
    }
}

#[cfg(test)]
mod releases_tests_online {
    use std::str::FromStr;
    use std::sync::Once;

    use crate::unity::{
        extract_release_notes, fetch_release_notes, fetch_update_info, http_cache, Version,
    };

    static INIT: Once = Once::new();

    pub(crate) fn initialize() {
        INIT.call_once(|| {
            http_cache::set_cache_enabled(false).unwrap();
        });
    }

    /// Scraping <https://unity.com/releases/editor/archive> for updates to 2019.1.0f1.
    /// At the time of writing, there were 15.
    #[test]
    fn test_request_updates_2019_1_0() {
        initialize();
        let v = Version::from_str("2019.1.0f1").unwrap();
        let (current, updates) = fetch_update_info(v).unwrap();
        current.unwrap();
        assert!(updates.len() >= 15);
    }

    /// Scraping <https://unity.com/releases/editor/archive> for updates to 5.0.0f1.
    /// At the time of writing, there were 19, and it is assumed that this will not change.
    #[test]
    fn test_request_updates_5_0_0() {
        initialize();
        let v = Version::from_str("5.0.0f1").unwrap();
        let (current, updates) = fetch_update_info(v).unwrap();
        // 5.0.0f1 does not have a release
        assert!(current.is_none());
        assert_eq!(updates.len(), 19);
    }

    #[test]
    fn test_release_notes_5_0_0() {
        initialize();
        let v = Version::from_str("5.0.0f1").unwrap();
        let (url, html) = &fetch_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 47, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 1114, "{url}");
    }

    #[test]
    fn test_release_notes_2017_1_0() {
        initialize();
        let v = Version::from_str("2017.1.0f3").unwrap();
        let (url, html) = &fetch_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 6, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 440, "{url}");
    }

    #[test]
    fn test_release_notes_2017_2_5() {
        initialize();
        let v = Version::from_str("2017.2.5f1").unwrap();
        let (url, html) = &fetch_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 1, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 10, "{url}");
    }

    #[test]
    fn test_release_notes_2021_3_17() {
        initialize();
        let v = Version::from_str("2021.3.17f1").unwrap();
        let (url, html) = &fetch_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 7, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 205, "{url}");
    }

    #[test]
    fn test_release_notes_2022_2_0() {
        initialize();
        let v = Version::from_str("2022.2.0f1").unwrap();
        let (url, html) = &fetch_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 7, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 2090, "{url}");
    }

    #[test]
    fn test_release_notes_2023_1_0b11() {
        initialize();
        let v = Version::from_str("2023.1.0b11").unwrap();
        let (url, html) = &fetch_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 7, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 2126, "{url}");
    }

    #[test]
    fn test_release_notes_2023_2_0a9() {
        initialize();
        let v = Version::from_str("2023.2.0a9").unwrap();
        let (url, html) = &fetch_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 7, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 892, "{url}");
    }
}
