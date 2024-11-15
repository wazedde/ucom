use crate::commands::term_stat::TermStat;
use crate::unity::http_cache::ucom_cache_dir;
use crate::unity::release_api_data::{ReleaseData, ReleaseDataPayload};
use crate::unity::{http_cache, Version};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use ureq::Agent;

const RELEASES_API_URL: &str = "https://services.api.unity.com/unity/editor/release/v1/releases";
const RELEASES_FILENAME: &str = "releases.json";

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Releases {
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

/// Download release information from the Unity Release API.
pub fn download_release_info<F>(releases: &mut Releases, callback: F) -> anyhow::Result<usize>
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
        let response = Agent::new()
            .get(RELEASES_API_URL)
            .query("limit", &limit.to_string())
            .query("offset", &offset.to_string())
            .query("order", "RELEASE_DATE_DESC")
            .call()?;

        let payload: ReleaseDataPayload = serde_json::from_str(&response.into_string()?)?;
        total = payload.total;

        if payload.results.is_empty() {
            break;
        }

        for release in payload.results {
            if release.release_date <= latest_release_date {
                return Ok(fetched);
            }
            if release.recommended.unwrap_or(false) {
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

/// Loads the release info from the cache.
/// Releases are sorted by release date in descending order.
pub fn load_release_info(path: &PathBuf) -> anyhow::Result<Releases> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let releases: Releases = serde_json::from_reader(reader)?;
    Ok(releases)
}

pub fn load_cached_releases() -> anyhow::Result<Releases> {
    let path = ucom_cache_dir().join(RELEASES_FILENAME);
    if path.exists() {
        load_release_info(&path)
    } else {
        Ok(Releases::default())
    }
}

/// Downloads and caches the release info. List is sorted by version in ascending order.
pub fn get_latest_releases() -> anyhow::Result<Releases> {
    let releases_path = ucom_cache_dir().join(RELEASES_FILENAME);
    let mut releases = load_cached_releases()?;

    if http_cache::has_expired(&releases_path) {
        let ts = TermStat::new("Download", "Release info");
        let fetch_count = download_release_info(&mut releases, |count: usize, total: usize| {
            let percentage = count as f64 / total as f64 * 100.0;
            ts.reprint("Download", &format!("Release info ({:.2}%)", percentage));
        })?;

        if fetch_count > 0 {
            // Sort ascending by version
            releases.releases.sort_unstable_by_key(|r| r.version);
            releases.last_updated = Utc::now();

            create_dir_all(ucom_cache_dir())?;
            serde_json::to_writer(
                BufWriter::new(File::create(&releases_path)?),
                &json!(releases),
            )?;
        } else {
            http_cache::touch_timestamp(&releases_path)?;
        }
    }
    Ok(releases)
}
