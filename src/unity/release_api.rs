use crate::commands::term_stat::TermStat;
use crate::unity::http_cache;
use crate::unity::http_cache::ucom_cache_dir;
use crate::unity::release_api_data::{ReleaseData, ReleaseDataPayload};
use serde_json::json;
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use ureq::Agent;

const RELEASES_API_URL: &str = "https://services.api.unity.com/unity/editor/release/v1/releases";
const RELEASES_FILENAME: &str = "releases.json";

/// Download release information from the Unity Release API.
pub fn download_release_info<F>(
    releases: &mut Vec<ReleaseData>,
    callback: F,
) -> anyhow::Result<usize>
where
    F: Fn(usize, usize),
{
    let mut fetched = 0;
    let mut offset = 0;

    let limit = if releases.is_empty() { 25 } else { 5 };
    let latest_release_date = releases.first().map(|d| d.release_date).unwrap_or_default();

    'outer: loop {
        let response = Agent::new()
            .get(RELEASES_API_URL)
            .query("limit", &limit.to_string())
            .query("offset", &offset.to_string())
            .query("order", "RELEASE_DATE_DESC")
            .call()?;

        let payload: ReleaseDataPayload =
            serde_json::from_str(&response.into_string()?).expect("JSON was not well-formed");
        let total = payload.total;

        if payload.results.is_empty() {
            break 'outer;
        }

        for release in payload.results {
            if release.release_date <= latest_release_date {
                break 'outer;
            }
            releases.push(release);
            fetched += 1;
        }

        offset += limit;
        callback(fetched, total);

        if offset >= total {
            break 'outer;
        }
    }

    Ok(fetched)
}

/// Loads the release info from the cache.
/// Releases are sorted by release date in descending order.
pub fn load_release_info(path: &PathBuf) -> anyhow::Result<Vec<ReleaseData>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let releases: Vec<ReleaseData> = serde_json::from_reader(reader)?;
    Ok(releases)
}

/// Downloads and caches the release info. List is sorted by version in ascending order.
pub fn load_and_download_release_info() -> anyhow::Result<Vec<ReleaseData>> {
    let path = ucom_cache_dir().join(RELEASES_FILENAME);
    let mut releases = load_release_info(&path).unwrap_or_default();

    let ts = TermStat::new("Download", "Release info");
    let callback = |count: usize, total: usize| {
        let percentage = count as f64 / total as f64 * 100.0;
        ts.reprint("Download", &format!("Release info ({:.2}%)", percentage));
    };

    if http_cache::has_expired(&path) {
        let fetch_count = download_release_info(&mut releases, callback)?;
        if fetch_count > 0 {
            create_dir_all(ucom_cache_dir())?;
            let path = ucom_cache_dir().join(RELEASES_FILENAME);
            let writer = BufWriter::new(File::create(&path)?);
            serde_json::to_writer(writer, &json!(releases))?;
        } else {
            http_cache::touch_timestamp(&path)?;
        }
    }
    // Sort ascending by version
    releases.sort_unstable_by_key(|r| r.version);
    Ok(releases)
}
