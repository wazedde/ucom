use std::borrow::Cow;

use anyhow::Result;
use indexmap::IndexMap;
use select::document::Document;
use select::predicate::{Class, Name};

use crate::unity::{BuildType, MajorVersion, MinorVersion, UnityVersion};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone)]
pub struct ReleaseInfo {
    pub version: UnityVersion,
    pub date_header: String,
    pub installation_url: String,
}

impl ReleaseInfo {
    const fn new(version: UnityVersion, date_header: String, installation_url: String) -> Self {
        Self {
            version,
            date_header,
            installation_url,
        }
    }
}

#[allow(dead_code)]
pub enum ReleaseFilter {
    /// Match all releases.
    All,
    /// Match releases on major version.
    Major { major: MajorVersion },
    /// Match releases on major and minor version.
    Minor {
        major: MajorVersion,
        minor: MinorVersion,
    },
}

impl ReleaseFilter {
    const fn eval(&self, v: UnityVersion) -> bool {
        match self {
            Self::All => true,
            Self::Major { major } => v.major == *major,
            Self::Minor { major, minor } => v.major == *major && v.minor == *minor,
        }
    }
}

/// Gets Unity releases from the Unity website.
pub fn request_unity_releases() -> Result<Vec<ReleaseInfo>> {
    let url = "https://unity.com/releases/editor/archive";
    let body = ureq::get(url).call()?.into_string()?;
    let releases = extract_releases(&body, &ReleaseFilter::All);
    Ok(releases)
}

/// Gets patch updates for the given version from the Unity website.
pub fn request_patch_update_info(
    version: UnityVersion,
) -> Result<(Option<ReleaseInfo>, Vec<ReleaseInfo>)> {
    let url = "https://unity.com/releases/editor/archive";
    let body = ureq::get(url).call()?.into_string()?;

    let releases = extract_releases(
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

pub fn request_release_notes(version: UnityVersion) -> Result<(String, String)> {
    let url = release_notes_url(version);
    let body = ureq::get(&url).call()?.into_string()?;
    Ok((url, body))
}

/// Extracts releases from the html that match the filter.
fn extract_releases(html: &str, filter: &ReleaseFilter) -> Vec<ReleaseInfo> {
    let major_release_class: Cow<'_, str> = match filter {
        ReleaseFilter::All => "release-tab-content".into(),
        ReleaseFilter::Major { major } | ReleaseFilter::Minor { major, .. } => {
            major.to_string().into()
        }
    };

    let mut versions: Vec<_> = Document::from(html)
        .find(Class(major_release_class.as_ref()))
        .flat_map(|n| n.find(Class("download-release-wrapper")))
        .filter_map(|n| {
            n.find(Class("release-title-date"))
                .next()
                // Get the release date.
                .and_then(|n| n.find(Class("release-date")).next())
                .map(|n| n.text())
                .and_then(|date_header| {
                    // Get the Unity Hub installation url, which is the next sibling of the button.
                    n.find(Class("btn"))
                        .next()
                        .and_then(|n| n.attr("href"))
                        .map(|url| (date_header, url))
                })
        })
        // Filter out releases that don't match the filter.
        .filter_map(|(date_header, url)| {
            version_from_url(url)
                .filter(|&v| filter.eval(v))
                .map(|version| ReleaseInfo::new(version, date_header, url.to_owned()))
        })
        .collect();

    versions.sort_unstable();
    versions
}

/// Get the version from the url.
/// The url looks like: `unityhub://2021.2.14f1/bcb93e5482d2`
fn version_from_url(url: &str) -> Option<UnityVersion> {
    url.split('/')
        .rev()
        .nth(1)
        .and_then(|v| v.parse::<UnityVersion>().ok())
}

pub fn release_notes_url(version: UnityVersion) -> String {
    match version.build_type {
        BuildType::Alpha => format!("https://unity.com/releases/editor/alpha/{version}"),
        BuildType::Beta => format!("https://unity.com/releases/editor/beta/{version}"),
        BuildType::Final | BuildType::ReleaseCandidate => {
            let version = if version.build == 1 {
                // 2021.2.1f1 -> "2021.2.1"
                format!("{}.{}.{}", version.major, version.minor, version.patch)
            } else {
                // 5.1.0f2 -> "5.1.0-0"
                format!(
                    "{}.{}.{}-{}",
                    version.major,
                    version.minor,
                    version.patch,
                    version.build - 2
                )
            };

            format!("https://unity.com/releases/editor/whats-new/{version}")
        }
    }
}

/// Extracts release notes from the supplied html.
pub fn extract_release_notes(html: &str) -> IndexMap<String, Vec<String>> {
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
                if !release_notes.contains_key(&topic_header) {
                    release_notes.insert(topic_header.clone(), Vec::new());
                }

                // Iterate over the list items and add them to the topic list.
                let topic_list = release_notes.get_mut(&topic_header).unwrap();
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

    use crate::unity::{ReleaseFilter, UnityVersion};

    use super::{extract_releases, version_from_url};

    #[test]
    fn test_version_from_url() {
        let url = "unityhub://2021.2.14f1/bcb93e5482d2";
        let version = version_from_url(url).unwrap();
        assert_eq!(version, UnityVersion::from_str("2021.2.14f1").unwrap());
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
        let releases = extract_releases(html, &ReleaseFilter::All);
        assert_eq!(releases.len(), 473);
    }

    #[test]
    fn test_find_releases_major() {
        let html = include_str!("test_data/unity_download_archive.html");
        let releases = extract_releases(html, &ReleaseFilter::Major { major: 2021 });
        assert_eq!(releases.len(), 66);
    }

    #[test]
    fn test_find_releases_minor() {
        let html = include_str!("test_data/unity_download_archive.html");
        let releases = extract_releases(
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
        let version = UnityVersion::from_str("2021.2.14f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(url, "https://unity.com/releases/editor/whats-new/2021.2.14");
    }

    #[test]
    fn test_release_notes_url_5_1_0_1() {
        let version = UnityVersion::from_str("5.1.0f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(url, "https://unity.com/releases/editor/whats-new/5.1.0");
    }

    #[test]
    fn test_release_notes_url_5_1_0_2() {
        let version = UnityVersion::from_str("5.1.0f2").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(url, "https://unity.com/releases/editor/whats-new/5.1.0-0");
    }

    #[test]
    fn test_release_notes_url_5_1_0_3() {
        let version = UnityVersion::from_str("5.1.0f3").unwrap();
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

    use crate::unity::{
        extract_release_notes, request_patch_update_info, request_release_notes, UnityVersion,
    };

    /// Scraping https://unity.com/releases/editor/archive for updates to 2019.1.0f1.
    /// At the time of writing there were 15.
    #[test]
    fn test_request_updates_2019_1_0() {
        let v = UnityVersion::from_str("2019.1.0f1").unwrap();
        let (_, updates) = request_patch_update_info(v).unwrap();

        assert!(updates.len() >= 15);
    }

    /// Scraping https://unity.com/releases/editor/archive for updates to 5.0.0f1.
    /// At the time of writing there were 5 and it is assumed that this will not change.
    #[test]
    fn test_request_updates_5_0_0() {
        let v = UnityVersion::from_str("5.0.0f1").unwrap();
        let (_, updates) = request_patch_update_info(v).unwrap();

        assert_eq!(updates.len(), 5);
    }

    #[test]
    fn test_release_notes_5_0_0() {
        let v = UnityVersion::from_str("5.0.0f1").unwrap();
        let (url, html) = &request_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 47, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 1114, "{url}");
    }

    #[test]
    fn test_release_notes_2017_1_0() {
        let v = UnityVersion::from_str("2017.1.0f3").unwrap();
        let (url, html) = &request_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 6, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 440, "{url}");
    }

    #[test]
    fn test_release_notes_2017_2_5() {
        let v = UnityVersion::from_str("2017.2.5f1").unwrap();
        let (url, html) = &request_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 1, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 10, "{url}");
    }

    #[test]
    fn test_release_notes_2021_3_17() {
        let v = UnityVersion::from_str("2021.3.17f1").unwrap();
        let (url, html) = &request_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 7, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 205, "{url}");
    }

    #[test]
    fn test_release_notes_2022_2_0() {
        let v = UnityVersion::from_str("2022.2.0f1").unwrap();
        let (url, html) = &request_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 7, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 2090, "{url}");
    }

    #[test]
    fn test_release_notes_2023_1_0b11() {
        let v = UnityVersion::from_str("2023.1.0b11").unwrap();
        let (url, html) = &request_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 7, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 2126, "{url}");
    }

    #[test]
    fn test_release_notes_2023_2_0a9() {
        let v = UnityVersion::from_str("2023.2.0a9").unwrap();
        let (url, html) = &request_release_notes(v).unwrap();

        let release_notes = extract_release_notes(html);
        assert_eq!(release_notes.len(), 7, "{url}");
        assert_eq!(release_notes.values().flatten().count(), 892, "{url}");
    }
}
