use crate::unity::release_api::{Mode, SortedReleaseCollection, fetch_latest_releases};
use crate::unity::release_api_data::ReleaseData;
use crate::unity::{BuildType, Major, Minor, Version};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
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
    #[strum(serialize = "    ")]
    Other,
}

pub(crate) enum ReleaseCriteria {
    /// Match all releases.
    #[allow(dead_code)]
    All,
    /// Match releases on the major version.
    #[allow(dead_code)]
    Major { major: Major },
    /// Match releases on the major and minor version.
    #[allow(dead_code)]
    Minor { major: Major, minor: Minor },
}

impl ReleaseCriteria {
    /// Returns true if the version matches the filter.
    const fn is_version_match(&self, v: Version) -> bool {
        match self {
            Self::All => true,
            Self::Major { major } => v.major == *major,
            Self::Minor { major, minor } => v.major == *major && v.minor == *minor,
        }
    }
}

pub(crate) struct ReleaseUpdates {
    pub(crate) current_release: ReleaseData,
    pub(crate) newer_releases: SortedReleaseCollection,
}

pub(crate) fn find_available_updates(
    version: Version,
    mode: Mode,
) -> anyhow::Result<ReleaseUpdates> {
    let releases = fetch_latest_releases(mode)?;
    let criteria = ReleaseCriteria::Minor {
        major: version.major,
        minor: version.minor,
    };

    let mut releases = releases.filter(|rd| criteria.is_version_match(rd.version));
    let position = releases.iter().position(|rd| rd.version == version);
    let current_release = position
        .map(|index| releases.remove(index))
        .ok_or_else(|| anyhow::anyhow!("Version {} not found in releases", version))?;
    let newer_releases = releases.filter(|rd| rd.version > version);

    Ok(ReleaseUpdates {
        current_release,
        newer_releases,
    })
}

pub(crate) struct Url(String);

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

pub(crate) fn release_notes_url(version: Version) -> Url {
    match version.build_type {
        BuildType::Alpha => Url(format!(
            "https://unity.com/releases/editor/alpha/{version}#notes"
        )),
        BuildType::Beta => Url(format!(
            "https://unity.com/releases/editor/beta/{version}#notes"
        )),
        BuildType::Final | BuildType::ReleaseCandidate => {
            let version = format!("{}.{}.{}", version.major, version.minor, version.patch);
            Url(format!(
                "https://unity.com/releases/editor/whats-new/{version}#notes"
            ))
        }
        BuildType::FinalPatch => Url(format!(
            "https://unity.com/releases/editor/whats-new/{version}#notes"
        )),
    }
}

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
            "https://unity.com/releases/editor/whats-new/2021.2.14#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_1() {
        let version = Version::from_str("5.1.0f1").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url.as_ref(),
            "https://unity.com/releases/editor/whats-new/5.1.0#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_2() {
        let version = Version::from_str("5.1.0f2").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url.as_ref(),
            "https://unity.com/releases/editor/whats-new/5.1.0#notes"
        );
    }

    #[test]
    fn test_release_notes_url_5_1_0_3() {
        let version = Version::from_str("5.1.0f3").unwrap();
        let url = super::release_notes_url(version);
        assert_eq!(
            url.as_ref(),
            "https://unity.com/releases/editor/whats-new/5.1.0#notes"
        );
    }
}
