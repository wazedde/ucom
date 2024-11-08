use crate::unity::release_api::load_and_download_release_info;
use crate::unity::release_api_data::ReleaseData;
use crate::unity::{BuildType, Major, Minor, Version};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Display, Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy)]
pub(crate) enum ReleaseStream {
    #[serde(rename = "LTS")]
    #[strum(serialize = "LTS")]
    Lts,
    #[serde(rename = "TECH")]
    #[strum(serialize = "TECH")]
    Tech,
    #[serde(rename = "BETA")]
    #[strum(serialize = "BETA")]
    Beta,
    #[serde(rename = "ALPHA")]
    #[strum(serialize = "ALPHA")]
    Alpha,
    #[serde(other)]
    #[strum(serialize = "????")]
    Other,
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
pub(crate) fn fetch_unity_editor_releases() -> anyhow::Result<Vec<ReleaseData>> {
    load_and_download_release_info()
}

/// Gets the current and update releases for the given version from the Unity website.
pub(crate) fn fetch_update_info(
    version: Version,
) -> anyhow::Result<(Option<ReleaseData>, Vec<ReleaseData>)> {
    let releases = load_and_download_release_info()?;
    let filter = ReleaseFilter::Minor {
        major: version.major,
        minor: version.minor,
    };

    let releases = releases
        .into_iter()
        .filter(|rd| filter.eval(rd.version))
        .collect_vec();

    let current = releases.iter().find(|ri| ri.version == version).cloned();
    let updates = releases
        .into_iter()
        .filter(|ri| ri.version > version) // Only newer versions.
        .collect();

    Ok((current, updates))
}

pub(crate) type Url = String;

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

#[cfg(test)]
mod releases_tests {
    use std::str::FromStr;

    use crate::unity::Version;

    use super::version_from_url;

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
    fn test_release_notes_url() {
        let version = Version::from_str("2021.2.14f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url,
            "https://unity.com/releases/editor/whats-new/2021.2.14#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_1() {
        let version = Version::from_str("5.1.0f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url,
            "https://unity.com/releases/editor/whats-new/5.1.0#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_2() {
        let version = Version::from_str("5.1.0f2").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url,
            "https://unity.com/releases/editor/whats-new/5.1.0#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_3() {
        let version = Version::from_str("5.1.0f3").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url,
            "https://unity.com/releases/editor/whats-new/5.1.0#notes"
        );
    }
}

#[cfg(test)]
mod releases_tests_online {
    use std::str::FromStr;
    use std::sync::Once;

    use crate::unity::{fetch_update_info, http_cache, Version};

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
}
