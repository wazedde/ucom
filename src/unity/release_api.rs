use crate::commands::status_line::StatusLine;
use crate::unity::content_cache::get_cache_dir;
use crate::unity::release_api_data::{ReleaseData, ReleaseDataPage};
use crate::unity::{content_cache, Version};
use anyhow::Context;
use chrono::{DateTime, Utc};
use content_cache::{is_expired, update_timestamp};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter};
use std::ops::Deref;
use std::path::Path;

const RELEASES_API_URL: &str = "https://services.api.unity.com/unity/editor/release/v1/releases";
const RELEASES_FILENAME: &str = "releases_dataset.json";

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Releases {
    #[serde(rename = "lastUpdated")]
    pub last_updated: DateTime<Utc>,
    #[serde(rename = "suggestedVersion")]
    pub suggested_version: Option<Version>,

    #[serde(rename = "releases")]
    releases: Vec<ReleaseData>,
}

impl Releases {
    pub(crate) fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }

    pub(crate) fn has_version(&self, version: Version) -> bool {
        // TODO: Use a HashSet for faster lookups
        self.releases.iter().any(|r| r.version == version)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &ReleaseData> {
        self.releases.iter()
    }

    pub(crate) fn into_iter(self) -> impl Iterator<Item = ReleaseData> {
        self.releases.into_iter()
    }
}

impl Default for Releases {
    fn default() -> Self {
        Releases {
            last_updated: Utc::now(),
            suggested_version: None,
            releases: Vec::new(),
        }
    }
}

/// Wrapper around the releases that sorts the releases by version in ascending order.
pub(crate) struct SortedReleases(Releases);

impl Deref for SortedReleases {
    type Target = Releases;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Sort list when creating struct
#[allow(dead_code)]
impl SortedReleases {
    /// Sorts the releases by version in ascending order.
    pub(crate) fn new(mut releases: Releases) -> Self {
        releases.releases.sort_unstable_by_key(|r| r.version);
        SortedReleases(releases)
    }

    pub(crate) fn into_inner(self) -> Releases {
        self.0
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &ReleaseData> {
        self.0.iter()
    }

    /// Returns filtered releases.
    pub(crate) fn filtered<F>(self, predicate: F) -> Self
    where
        F: Fn(&ReleaseData) -> bool,
    {
        let last_updated = self.0.last_updated;
        let suggested_version = self.0.suggested_version;
        let releases = self.0.into_iter().filter(|r| predicate(r)).collect_vec();
        SortedReleases(Releases {
            last_updated,
            suggested_version,
            releases,
        })
    }

    /// Removes and returns the release at the given index.
    pub(crate) fn remove(&mut self, index: usize) -> ReleaseData {
        self.0.releases.remove(index)
    }
}

/// Fetches a page of releases from the Unity Release API in descending order by release date.
fn fetch_releases_page(limit: usize, offset: usize) -> anyhow::Result<ReleaseDataPage> {
    let body = ureq::get(RELEASES_API_URL)
        .query("limit", limit.to_string())
        .query("offset", offset.to_string())
        .query("order", "RELEASE_DATE_DESC")
        .call()
        .context("Failed to fetch release data")?
        .into_body()
        .into_reader();

    serde_json::from_reader(body).context("Failed to parse release data")
}

/// Download release information from the Unity Release API.
/// Because the API is very slow, we minimize the number of requests when looking for new releases
/// by assuming there were no new releases if all releases in a page are already in the list.
/// This is not perfect, but seems to be good enough for our use case.
/// practice earlier releases can be added later.
pub(crate) fn download_release_info<F>(
    releases: &mut Releases,
    callback: F,
) -> anyhow::Result<usize>
where
    F: Fn(usize, usize),
{
    // If list is empty, fetch as much as possible, otherwise fetch 5 at a time to make it faster.
    let fetch_all = releases.is_empty();
    let limit = if fetch_all { 25 } else { 5 };

    let mut fetched = 0;
    let mut offset = 0;
    let mut total = usize::MAX;
    let mut fetched_in_page;

    while offset < total {
        let page = fetch_releases_page(limit, offset)?;
        total = page.total;
        fetched_in_page = 0;

        if page.results.is_empty() {
            // No more releases to fetch
            break;
        }

        for release in page.results {
            if !fetch_all && releases.has_version(release.version) {
                continue;
            }

            if release.recommended == Some(true) {
                releases.suggested_version = Some(release.version);
            }

            releases.releases.push(release);
            fetched += 1;
            fetched_in_page += 1;
        }

        if fetched_in_page == 0 {
            // No new releases in this page, assume we have fetched all new releases.
            break;
        }

        offset += limit;
        callback(fetched, total);
    }

    Ok(fetched)
}

/// Load release info from a file.
fn load_release_info(path: &Path) -> anyhow::Result<Releases> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let releases: Releases = serde_json::from_reader(reader)?;
    Ok(releases)
}

/// Loads the release info from the cache.
pub(crate) fn load_cached_releases() -> anyhow::Result<Releases> {
    let path = get_cache_dir().join(RELEASES_FILENAME);
    if path.exists() {
        load_release_info(&path)
    } else {
        Ok(Releases::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Auto,
    Force,
}

/// Downloads and caches the release info.
pub(crate) fn get_latest_releases(mode: Mode) -> anyhow::Result<SortedReleases> {
    let releases_path = get_cache_dir().join(RELEASES_FILENAME);
    let mut releases = if mode == Mode::Auto {
        load_cached_releases()?
    } else {
        Releases::default()
    };

    if mode == Mode::Auto && !is_expired(&releases_path) {
        return Ok(SortedReleases::new(releases));
    }

    let download_status = StatusLine::new("Downloading", "release data...");
    let fetch_count = download_release_info(&mut releases, |count, total| {
        let percentage = count as f64 / total as f64 * 100.0;
        download_status.update("Downloading", &format!("release data ({:.2}%)", percentage));
    })?;

    if fetch_count > 0 {
        releases.last_updated = Utc::now();
        let sorted_releases = SortedReleases::new(releases);

        create_dir_all(get_cache_dir())?;
        serde_json::to_writer(
            BufWriter::new(File::create(&releases_path)?),
            &json!(sorted_releases.0),
        )?;
        Ok(sorted_releases)
    } else {
        update_timestamp(&releases_path)?;
        Ok(SortedReleases::new(releases))
    }
}
