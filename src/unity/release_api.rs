use crate::unity::Version;
use crate::unity::release_api_data::{ReleaseData, ReleaseDataPage};
use crate::utils::content_cache::ucom_cache_dir;
use crate::utils::content_cache::{is_cache_file_expired, touch_file};
use crate::utils::status_line::StatusLine;
use anyhow::Context;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter};
use std::path::Path;

const RELEASES_API_URL: &str = "https://services.api.unity.com/unity/editor/release/v1/releases";
const RELEASES_FILENAME: &str = "releases_dataset.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseCollection {
    #[serde(rename = "lastUpdated")]
    pub last_updated: DateTime<Utc>,
    #[serde(rename = "suggestedVersion")]
    pub suggested_version: Option<Version>,

    #[serde(rename = "releases")]
    releases: Vec<ReleaseData>,
}

impl IntoIterator for ReleaseCollection {
    type Item = ReleaseData;
    type IntoIter = std::vec::IntoIter<ReleaseData>;

    fn into_iter(self) -> Self::IntoIter {
        self.releases.into_iter()
    }
}

impl Default for ReleaseCollection {
    fn default() -> Self {
        Self {
            last_updated: Utc::now(),
            suggested_version: None,
            releases: Vec::new(),
        }
    }
}

impl ReleaseCollection {
    /// Returns if the collection has a release with the given version.
    pub fn has_version(&self, version: Version) -> bool {
        // No need to cache hits as each check is done once during the lifetime of the program.
        self.iter().any(|r| r.version == version)
    }

    pub fn find_by_version(&self, version: Version) -> Option<&ReleaseData> {
        self.iter().find(|r| r.version == version)
    }

    /// Returns the release with the given version.
    /// # Panics if the version is not found.
    pub fn get_by_version(&self, version: Version) -> &ReleaseData {
        self.find_by_version(version)
            .unwrap_or_else(|| panic!("Release with version {version} not found in collection"))
    }

    pub fn iter(&self) -> impl Iterator<Item = &ReleaseData> {
        self.releases.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }
}

/// Wrapper around the releases that sorts the releases by version in ascending order.
pub struct SortedReleaseCollection(ReleaseCollection);

impl AsRef<ReleaseCollection> for SortedReleaseCollection {
    fn as_ref(&self) -> &ReleaseCollection {
        &self.0
    }
}

impl From<SortedReleaseCollection> for ReleaseCollection {
    fn from(sorted: SortedReleaseCollection) -> Self {
        sorted.0
    }
}

// Sort list when creating struct
impl SortedReleaseCollection {
    /// Sorts the releases by version in ascending order.
    pub fn new(mut collection: ReleaseCollection) -> Self {
        collection.releases.sort_unstable_by_key(|r| r.version);
        Self(collection)
    }

    /// Returns filtered releases.
    pub fn filter<F>(self, predicate: F) -> Self
    where
        F: Fn(&ReleaseData) -> bool,
    {
        Self(ReleaseCollection {
            last_updated: self.last_updated(),
            suggested_version: self.suggested_version(),
            releases: self.0.into_iter().filter(predicate).collect_vec(),
        })
    }

    pub fn last_updated(&self) -> DateTime<Utc> {
        self.0.last_updated
    }

    pub fn suggested_version(&self) -> Option<Version> {
        self.0.suggested_version
    }

    /// Removes and returns the release at the given index.
    pub fn remove(&mut self, index: usize) -> ReleaseData {
        self.0.releases.remove(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ReleaseData> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the release with the given version.
    pub fn get_by_version(&self, version: Version) -> &ReleaseData {
        self.0.get_by_version(version)
    }
}

/// Loads the release info from the cache.
pub fn load_cached_releases() -> anyhow::Result<ReleaseCollection> {
    let path = ucom_cache_dir()?.join(RELEASES_FILENAME);
    if path.exists() {
        load_release_info(&path)
    } else {
        Ok(ReleaseCollection::default())
    }
}

/// Fetch mode for fetching releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchMode {
    /// Fetch only if the cache is expired.
    Auto,
    /// Fetch regardless of the cache.
    Force,
}

/// Downloads and caches the release info.
pub fn fetch_latest_releases(mode: FetchMode) -> anyhow::Result<SortedReleaseCollection> {
    let releases_path = ucom_cache_dir()?.join(RELEASES_FILENAME);
    let mut releases = if mode == FetchMode::Auto {
        load_cached_releases()?
    } else {
        ReleaseCollection::default()
    };

    if mode == FetchMode::Auto && !is_cache_file_expired(&releases_path) {
        return Ok(SortedReleaseCollection::new(releases));
    }

    let status = StatusLine::new("Downloading", "Unity release data...");
    let fetch_count = fetch_release_info(&mut releases, |count, total| {
        let percentage = count as f64 / total as f64 * 100.0;
        status.update(
            "Downloading",
            &format!("Unity release data ({percentage:.0}%)"),
        );
    })?;

    if fetch_count > 0 {
        releases.last_updated = Utc::now();
        let sorted_releases = SortedReleaseCollection::new(releases);

        create_dir_all(ucom_cache_dir()?)?;
        serde_json::to_writer(
            BufWriter::new(File::create(&releases_path)?),
            &json!(sorted_releases.0),
        )?;
        Ok(sorted_releases)
    } else {
        touch_file(&releases_path)?;
        Ok(SortedReleaseCollection::new(releases))
    }
}

/// Fetches a page of releases from the Unity Release API in descending order by release date.
fn fetch_releases_page(limit: usize, offset: usize) -> anyhow::Result<ReleaseDataPage> {
    let body = ureq::get(RELEASES_API_URL)
        .query("limit", limit.to_string())
        .query("offset", offset.to_string())
        .query("order", "RELEASE_DATE_DESC")
        .call()
        .context("Failed to fetch Unity release data")?
        .into_body()
        .into_reader();

    serde_json::from_reader(body).context("Failed to parse Unity release data")
}

/// Download release information from the Unity Release API.
/// Because the API is very slow, we minimize the number of requests when looking for new releases
/// by assuming there were no new releases if all releases in a page are already in the list.
/// This is not perfect, but seems to be good enough for our use case.
/// practice earlier releases can be added later.
fn fetch_release_info<F>(releases: &mut ReleaseCollection, callback: F) -> anyhow::Result<usize>
where
    F: Fn(usize, usize),
{
    // If list is empty, fetch as much as possible, otherwise fetch 5 at a time to make it faster.
    let fetch_all = releases.is_empty();
    let limit = if fetch_all { 25 } else { 5 };

    let mut fetched = 0;
    let mut offset = 0;
    let mut total = usize::MAX;

    while offset < total {
        let page = fetch_releases_page(limit, offset)?;
        total = page.total;
        let mut fetched_in_page = 0;

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
fn load_release_info(path: &Path) -> anyhow::Result<ReleaseCollection> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let releases: ReleaseCollection = serde_json::from_reader(reader)?;
    Ok(releases)
}
