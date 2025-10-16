use crate::unity::release_api::{SortedReleases, UpdatePolicy, fetch_latest_releases};
use crate::unity::release_api_data::ReleaseData;
use crate::unity::{BuildType, Version};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use strum::Display;

/// The release stream.
#[derive(Display, Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy)]
pub enum ReleaseStream {
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

    #[serde(rename = "SUPPORTED")]
    #[strum(serialize = "SUPP")]
    Supp,

    // Fallback for unknown values.
    #[serde(other)]
    #[strum(serialize = "OTHER")]
    Other,
}

/// The current release and newer releases.
pub struct ReleaseUpdates {
    pub current_release: ReleaseData,
    pub newer_releases: SortedReleases,
}

/// Finds the available updates for the given version.
pub fn find_available_updates(
    version: Version,
    mode: UpdatePolicy,
) -> anyhow::Result<ReleaseUpdates> {
    let mut releases = fetch_latest_releases(mode)?;

    releases.retain(|rd| {
        rd.version.major == version.major
            && rd.version.minor == version.minor
            && rd.version >= version
    });

    let index = releases
        .iter()
        .position(|rd| rd.version == version)
        .ok_or_else(|| anyhow::anyhow!("Version {version} not found in releases"))?;
    let current_release = releases.remove(index);

    Ok(ReleaseUpdates {
        current_release,
        newer_releases: releases,
    })
}

//
// URL
//

pub struct Url(String);

impl AsRef<str> for Url {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Display for Url {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Returns the release notes URL for the given version.
pub fn release_notes_url(version: Version) -> Url {
    match version.build_type {
        BuildType::Alpha => Url(format!(
            "https://unity.com/releases/editor/alpha/{version}#notes"
        )),
        BuildType::Beta => Url(format!(
            "https://unity.com/releases/editor/beta/{version}#notes"
        )),
        BuildType::Final | BuildType::ReleaseCandidate => Url(format!(
            "https://unity.com/releases/editor/whats-new/{version}#notes"
        )),
        BuildType::FinalPatch => Url(format!(
            "https://unity.com/releases/editor/whats-new/{version}#notes"
        )),
    }
}

//
// Tests
//

#[cfg(test)]
mod releases_tests {
    use std::str::FromStr;

    use crate::unity::Version;

    #[test]
    fn test_release_notes_url() {
        let version = Version::from_str("2021.2.14f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url.as_ref(),
            "https://unity.com/releases/editor/whats-new/2021.2.14f1#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_1() {
        let version = Version::from_str("5.1.0f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url.as_ref(),
            "https://unity.com/releases/editor/whats-new/5.1.0f1#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_2() {
        let version = Version::from_str("5.1.0f2").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url.as_ref(),
            "https://unity.com/releases/editor/whats-new/5.1.0f2#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_3() {
        let version = Version::from_str("5.1.0f3").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url.as_ref(),
            "https://unity.com/releases/editor/whats-new/5.1.0f3#notes"
        );
    }
}
