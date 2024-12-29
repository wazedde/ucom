use crate::commands::term_stat::TermStat;
use crate::unity::http_cache::ucom_cache_dir;
use crate::unity::release_api_data::{ReleaseData, ReleaseDataPage};
use crate::unity::{http_cache, Version};
use anyhow::Context;
use chrono::{DateTime, Utc};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter};
use std::ops::Deref;
use std::path::Path;
use ureq::Agent;

const RELEASES_API_URL: &str = "https://services.api.unity.com/unity/editor/release/v1/releases";
const RELEASES_FILENAME: &str = "releases.json";

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
    let response = Agent::new()
        .get(RELEASES_API_URL)
        .query("limit", &limit.to_string())
        .query("offset", &offset.to_string())
        .query("order", "RELEASE_DATE_DESC")
        .call()
        .context("Failed to fetch release data")?
        .into_reader();

    serde_json::from_reader(response).context("Failed to parse release data")
}

/// Download release information from the Unity Release API.
pub(crate) fn download_release_info<F>(
    releases: &mut Releases,
    callback: F,
) -> anyhow::Result<usize>
where
    F: Fn(usize, usize),
{
    let latest_release_date = releases
        .iter()
        .max_by(|a, b| a.release_date.cmp(&b.release_date))
        .map(|d| d.release_date)
        .unwrap_or_default();

    // If list is empty, fetch as much as possible, otherwise fetch 5 at a time to make it faster.
    let limit = if releases.releases.is_empty() { 25 } else { 5 };
    let mut fetched = 0;
    let mut offset = 0;
    let mut total = usize::MAX;

    while offset < total {
        let page = fetch_releases_page(limit, offset)?;
        total = page.total;

        if page.results.is_empty() {
            break;
        }

        for release in page.results {
            if release.release_date <= latest_release_date {
                return Ok(fetched);
            }
            if release.recommended.unwrap_or_default() {
                releases.suggested_version = Some(release.version);
            }
            releases.releases.push(release);
            fetched += 1;
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
    let path = ucom_cache_dir().join(RELEASES_FILENAME);
    if path.exists() {
        load_release_info(&path)
    } else {
        Ok(Releases::default())
    }
}

/// Downloads and caches the release info.
pub(crate) fn get_latest_releases() -> anyhow::Result<SortedReleases> {
    let releases_path = ucom_cache_dir().join(RELEASES_FILENAME);
    let mut releases = load_cached_releases()?;

    if !http_cache::has_expired(&releases_path) {
        return Ok(SortedReleases::new(releases));
    }

    let download_status = TermStat::new("Downloading", "release data...");
    let fetch_count = download_release_info(&mut releases, |count, total| {
        let percentage = count as f64 / total as f64 * 100.0;
        download_status.reprint("Downloading", &format!("release data ({:.2}%)", percentage));
    })?;

    if fetch_count > 0 {
        releases.last_updated = Utc::now();
        let sorted_releases = SortedReleases::new(releases);

        create_dir_all(ucom_cache_dir())?;
        serde_json::to_writer(
            BufWriter::new(File::create(&releases_path)?),
            &json!(sorted_releases.0),
        )?;
        Ok(sorted_releases)
    } else {
        http_cache::touch_timestamp(&releases_path)?;
        Ok(SortedReleases::new(releases))
    }
}
