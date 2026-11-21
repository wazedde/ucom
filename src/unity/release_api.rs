use crate::unity::Version;
use crate::unity::release_api_data::{ReleaseData, ReleaseDataPage};
use crate::utils::content_cache::{
    http_agent, is_cached_file_stale, store_in_cache, touch_file, ucom_cache_dir,
};
use crate::utils::native_progress::{NativeProgressBar, NativeProgressState};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::StatusLine;
use crate::utils::warn_non_fatal;
use anyhow::{Context, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufReader, ErrorKind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const RELEASES_API_URL: &str = "https://services.api.unity.com/unity/editor/release/v1/releases";
const RELEASES_FILENAME: &str = "releases_dataset.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct Releases {
    #[serde(rename = "lastUpdate", default)]
    pub last_update: DateTime<Utc>,
    #[serde(rename = "lastFullUpdate", default)]
    pub last_full_update: DateTime<Utc>,
    #[serde(rename = "suggestedVersion")]
    pub suggested_version: Option<Version>,

    #[serde(rename = "releases")]
    releases: Vec<ReleaseData>,
}

impl IntoIterator for Releases {
    type Item = ReleaseData;
    type IntoIter = std::vec::IntoIter<ReleaseData>;

    fn into_iter(self) -> Self::IntoIter {
        self.releases.into_iter()
    }
}

impl Default for Releases {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            last_update: now,
            last_full_update: now,
            suggested_version: None,
            releases: Vec::new(),
        }
    }
}

impl Releases {
    /// Returns the release with the given version.
    pub fn get_by_version(&self, version: Version) -> anyhow::Result<&ReleaseData> {
        self.iter()
            .find(|r| r.version == version)
            .ok_or_else(|| anyhow!("Version {version} not found in releases"))
    }

    pub fn iter(&self) -> impl Iterator<Item = &ReleaseData> {
        self.releases.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }
}

/// Wrapper around the releases that sorts the releases by version in ascending order.
pub struct SortedReleases(Releases);

impl AsRef<Releases> for SortedReleases {
    fn as_ref(&self) -> &Releases {
        &self.0
    }
}

impl From<SortedReleases> for Releases {
    fn from(sorted: SortedReleases) -> Self {
        sorted.0
    }
}

// Sort list when creating struct
impl SortedReleases {
    /// Sorts the releases by version in ascending order.
    pub fn new(mut collection: Releases) -> Self {
        collection.releases.sort_unstable_by_key(|r| r.version);
        Self(collection)
    }

    /// Removes all releases that do not match the given predicate.
    pub fn retain<F>(&mut self, predicate: F)
    where
        F: Fn(&ReleaseData) -> bool,
    {
        self.0.releases.retain(predicate);
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
    pub fn get_by_version(&self, version: Version) -> anyhow::Result<&ReleaseData> {
        self.0.get_by_version(version)
    }
}

/// Loads the release info from the cache.
///
/// Returns an empty collection if there is no cache file or it cannot be opened.
/// A cache file that cannot be parsed is removed, so that the next fetch rebuilds it
/// instead of failing on every run. A file that cannot be opened is left in place,
/// since the cause may be temporary.
pub fn load_cached_releases() -> anyhow::Result<Releases> {
    let path = ucom_cache_dir()?.join(RELEASES_FILENAME);

    let file = match File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Releases::default()),
        Err(e) => {
            warn_non_fatal!(
                "Could not read the release data cache at {}: {e}",
                path.normalized_display()
            );
            return Ok(Releases::default());
        }
    };

    match serde_json::from_reader(BufReader::new(file)) {
        Ok(releases) => Ok(releases),
        Err(e) => {
            warn_non_fatal!(
                "Discarding unreadable release data cache, it will be downloaded again: {e}"
            );
            let _ = fs::remove_file(&path);
            Ok(Releases::default())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdatePolicy {
    /// Perform an incremental update if the cache is stale.
    Incremental,
    /// Force a full refresh, ignoring any existing cache.
    ForceRefresh,
}

/// Downloads and caches the release info.
///
/// Writing the cache is best-effort: if it fails, a warning is printed and the
/// downloaded data is still returned.
pub fn fetch_latest_releases(mode: UpdatePolicy) -> anyhow::Result<SortedReleases> {
    const EXPIRATION_DAYS: i64 = 30;
    let releases_path = ucom_cache_dir()?.join(RELEASES_FILENAME);
    let (mode, mut releases) = match mode {
        UpdatePolicy::Incremental => {
            let releases = load_cached_releases()?;
            if (Utc::now() - releases.last_full_update).num_days() > EXPIRATION_DAYS {
                // Data set is more than 30 days old, fetch full data.
                (UpdatePolicy::ForceRefresh, Releases::default())
            } else {
                // Data is recent, fetch incremental data.
                (UpdatePolicy::Incremental, releases)
            }
        }
        UpdatePolicy::ForceRefresh => (UpdatePolicy::ForceRefresh, Releases::default()),
    };

    if mode == UpdatePolicy::Incremental && !is_cached_file_stale(&releases_path) {
        return Ok(SortedReleases::new(releases));
    }

    let data_description = if releases.is_empty() {
        "Full Unity release data..."
    } else {
        "Incremental Unity release data..."
    };

    let mut progress = NativeProgressBar::new();
    let status = StatusLine::new("Downloading", data_description);
    let fetch_count = fetch_release_info(&mut releases, |count, total| {
        let percentage = if total == 0 {
            100.0
        } else {
            count.min(total) as f64 / total as f64 * 100.0
        };
        progress.update(NativeProgressState::Determinate(percentage as u8));
        status.update_line(
            "Downloading",
            format!("{data_description} ({percentage:.0}%)"),
        );
    })?;

    if fetch_count == 0 {
        // A failed timestamp update only means the update check is repeated next time.
        let _ = touch_file(&releases_path);
        return Ok(SortedReleases::new(releases));
    }

    releases.last_update = Utc::now();
    let sorted_releases = SortedReleases::new(releases);
    store_in_cache(&releases_path, |writer| {
        Ok(serde_json::to_writer(writer, &sorted_releases.0)?)
    });
    Ok(sorted_releases)
}

/// Number of releases per page in a full download, the most the API allows.
const FULL_PAGE_SIZE: usize = 25;
/// Number of releases per page in an incremental update, which usually needs only one page.
const INCREMENTAL_PAGE_SIZE: usize = 5;
/// Number of pages a full download fetches at the same time.
const PARALLEL_REQUESTS: usize = 10;
/// Time between the starts of two requests. The API allows 10 requests per second.
const REQUEST_INTERVAL: Duration = Duration::from_millis(125);
/// Time to wait before trying a request again that was refused because of the API's rate limit,
/// as the API asks. Each further try waits twice as long.
const RATE_LIMIT_RETRY_DELAY: Duration = Duration::from_secs(1);
/// How often a request refused because of the API's rate limit is tried again.
const RATE_LIMIT_RETRIES: u32 = 3;

/// Fetches a page of releases from the Unity Release API in descending order by release date.
fn fetch_releases_page(limit: usize, offset: usize) -> anyhow::Result<ReleaseDataPage> {
    // Read in one go: parsing from the decompressing reader would decompress byte by byte.
    let body = http_agent()
        .get(RELEASES_API_URL)
        .query("limit", limit.to_string())
        .query("offset", offset.to_string())
        .query("order", "RELEASE_DATE_DESC")
        .call()
        .context("Failed to fetch Unity release data")?
        .into_body()
        .read_to_vec()
        .context("Failed to read Unity release data")?;

    serde_json::from_slice(&body).context("Failed to parse Unity release data")
}

/// Download release information from the Unity Release API.
///
/// Without any releases, downloads all of them (`fetch_all_pages`). Otherwise fetches only the
/// newest: the API is very slow, so it assumes there were no new releases once a page holds
/// only releases that are already in the list. This is not perfect, but seems to be good enough
/// for our use case.
fn fetch_release_info<F>(releases: &mut Releases, callback: F) -> anyhow::Result<usize>
where
    F: FnMut(usize, usize),
{
    if releases.is_empty() {
        let pages = fetch_all_pages(
            fetch_releases_page,
            REQUEST_INTERVAL,
            RATE_LIMIT_RETRY_DELAY,
            callback,
        )?;
        Ok(add_releases(
            releases,
            pages.into_iter().flat_map(|page| page.results),
        ))
    } else {
        fetch_new_releases(releases, callback)
    }
}

/// Adds the releases that are newer than those in `releases`, one page at a time.
fn fetch_new_releases<F>(releases: &mut Releases, mut callback: F) -> anyhow::Result<usize>
where
    F: FnMut(usize, usize),
{
    let mut fetched = 0;
    let mut offset = 0;

    loop {
        let page = retry_when_rate_limited(RATE_LIMIT_RETRY_DELAY, || {
            fetch_releases_page(INCREMENTAL_PAGE_SIZE, offset)
        })?;
        let total = page.total;

        let added = add_releases(releases, page.results);
        if added == 0 {
            // No new releases in this page, assume we have fetched all new releases.
            break;
        }

        fetched += added;
        offset += INCREMENTAL_PAGE_SIZE;
        callback(fetched, total);
        if offset >= total {
            break;
        }
    }

    Ok(fetched)
}

/// Fetches every page of releases with `fetch_page(limit, offset)` and returns them in order.
///
/// The first page is fetched alone, for the total number of releases; the others in parallel,
/// started at most once per `request_interval`. A request refused because of the rate limit is
/// tried again after `retry_delay`. The first error stops the download. `callback` gets the
/// number of releases fetched so far and the total.
fn fetch_all_pages(
    fetch_page: impl Fn(usize, usize) -> anyhow::Result<ReleaseDataPage> + Sync,
    request_interval: Duration,
    retry_delay: Duration,
    mut callback: impl FnMut(usize, usize),
) -> anyhow::Result<Vec<ReleaseDataPage>> {
    let limiter = RateLimiter::new(request_interval);
    let fetch = |index: usize| {
        retry_when_rate_limited(retry_delay, || {
            limiter.wait();
            fetch_page(FULL_PAGE_SIZE, index * FULL_PAGE_SIZE)
        })
    };

    let first = fetch(0)?;
    let total = first.total;
    let page_count = total.div_ceil(FULL_PAGE_SIZE);
    let mut fetched = first.results.len();
    callback(fetched, total);
    let mut pages = vec![(0, first)];

    let next_page = AtomicUsize::new(1);
    let (sender, receiver) = mpsc::channel();

    thread::scope(|scope| -> anyhow::Result<()> {
        for _ in 0..PARALLEL_REQUESTS.min(page_count.saturating_sub(1)) {
            let sender = sender.clone();
            let (next_page, fetch) = (&next_page, &fetch);
            scope.spawn(move || {
                loop {
                    let index = next_page.fetch_add(1, Ordering::Relaxed);
                    // Sending fails once an error has stopped the download.
                    if index >= page_count || sender.send((index, fetch(index))).is_err() {
                        break;
                    }
                }
            });
        }
        // The loop below ends when the last worker drops its sender.
        drop(sender);

        // An error drops the receiver, so the workers stop after their current request.
        for (index, result) in receiver {
            let page = result?;
            fetched += page.results.len();
            callback(fetched, total);
            pages.push((index, page));
        }
        Ok(())
    })?;

    // A release published during the download shifts the later pages by one, which can push the
    // oldest releases past the last page. The pages fetched after it report the new total.
    let latest_total = pages
        .iter()
        .map(|(_, page)| page.total)
        .max()
        .unwrap_or(total);
    for index in page_count.max(1)..latest_total.div_ceil(FULL_PAGE_SIZE) {
        pages.push((index, fetch(index)?));
    }

    pages.sort_unstable_by_key(|(index, _)| *index);
    Ok(pages.into_iter().map(|(_, page)| page).collect())
}

/// Adds the releases that are not in `releases` yet, in order, and returns how many were added.
///
/// In a full download, a release published during the download shifts the later pages by one,
/// so a release can appear twice; only its first appearance is kept. The new release itself is
/// added by the next incremental update.
fn add_releases(releases: &mut Releases, new: impl IntoIterator<Item = ReleaseData>) -> usize {
    let mut known: HashSet<Version> = releases.iter().map(|release| release.version).collect();
    let mut added = 0;

    for release in new {
        if !known.insert(release.version) {
            continue;
        }

        if release.recommended == Some(true) {
            releases.suggested_version = Some(release.version);
        }

        releases.releases.push(release);
        added += 1;
    }

    added
}

/// Calls `fetch` until it succeeds or fails for another reason than the API's rate limit, at
/// most `RATE_LIMIT_RETRIES` more times. Waits `delay` before the first new try, and twice as
/// long before each one after it, so requests from other ucom processes can get through.
fn retry_when_rate_limited<T>(
    delay: Duration,
    mut fetch: impl FnMut() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    for retry in 0..RATE_LIMIT_RETRIES {
        match fetch() {
            Err(e) if is_rate_limited(&e) => thread::sleep(delay * 2u32.pow(retry)),
            result => return result,
        }
    }
    fetch()
}

/// Returns whether the request was refused because of the API's rate limit.
fn is_rate_limited(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ureq::Error>(),
        Some(ureq::Error::StatusCode(429))
    )
}

/// Spaces out the starts of requests made from several threads.
struct RateLimiter {
    interval: Duration,
    next_start: Mutex<Instant>,
}

impl RateLimiter {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            next_start: Mutex::new(Instant::now()),
        }
    }

    /// Waits until the next request may start.
    fn wait(&self) {
        let start = {
            let mut next_start = self
                .next_start
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let start = (*next_start).max(Instant::now());
            *next_start = start + self.interval;
            start
        };
        thread::sleep(start.saturating_duration_since(Instant::now()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    /// Fetches all pages of `api` without waiting between requests.
    fn fetch_all_now(
        api: impl Fn(usize, usize) -> anyhow::Result<ReleaseDataPage> + Sync,
        callback: impl FnMut(usize, usize),
    ) -> anyhow::Result<Vec<ReleaseDataPage>> {
        fetch_all_pages(api, Duration::ZERO, Duration::ZERO, callback)
    }

    fn release(version: &str, recommended: bool) -> ReleaseData {
        serde_json::from_value(serde_json::json!({
            "version": version,
            "releaseDate": "2026-11-19T11:21:37.382Z",
            "stream": "LTS",
            "releaseNotes": { "url": "https://example.com/notes.md", "type": "MD" },
            "shortRevision": "abcdef123456",
            "skuFamily": "CLASSIC",
            "unityHubDeepLink": "unityhub://x",
            "recommended": recommended
        }))
        .unwrap()
    }

    /// Returns the version of the `n`th release of the fake API, counted from the oldest.
    fn fake_version(n: usize) -> String {
        format!("2022.3.{}f{}", n % 100, n / 100 + 1)
    }

    /// Returns a fake API with `count` releases, newest first.
    fn fake_api(count: usize) -> impl Fn(usize, usize) -> anyhow::Result<ReleaseDataPage> + Sync {
        move |limit, offset| {
            let results = (offset..(offset + limit).min(count))
                .map(|i| release(&fake_version(count - 1 - i), false))
                .collect();
            Ok(ReleaseDataPage {
                offset,
                limit,
                total: count,
                results,
            })
        }
    }

    fn versions(pages: &[ReleaseDataPage]) -> Vec<String> {
        pages
            .iter()
            .flat_map(|p| &p.results)
            .map(|r| r.version.to_string())
            .collect()
    }

    #[test]
    fn fetch_all_pages_returns_every_page_in_order() {
        let mut progress = Vec::new();
        let pages = fetch_all_now(fake_api(130), |fetched, total| {
            progress.push((fetched, total));
        })
        .unwrap();

        assert_eq!(pages.len(), 6);
        let expected: Vec<_> = (0..130).rev().map(fake_version).collect();
        assert_eq!(versions(&pages), expected);
        assert_eq!(progress.last(), Some(&(130, 130)));
    }

    #[test]
    fn fetch_all_pages_with_a_single_page() {
        let pages = fetch_all_now(fake_api(3), |_, _| {}).unwrap();
        assert_eq!(versions(&pages).len(), 3);
    }

    #[test]
    fn fetch_all_pages_fetches_releases_pushed_past_the_last_page() {
        // 50 releases fill exactly 2 pages. One is published after the first page is fetched,
        // which pushes the oldest release to a third page.
        let before = fake_api(50);
        let after = fake_api(51);
        let first_fetched = AtomicBool::new(false);
        let pages = fetch_all_now(
            |limit, offset| {
                if first_fetched.swap(true, Ordering::Relaxed) {
                    after(limit, offset)
                } else {
                    before(limit, offset)
                }
            },
            |_, _| {},
        )
        .unwrap();
        let mut releases = Releases::default();

        let added = add_releases(&mut releases, pages.into_iter().flat_map(|p| p.results));

        // The new release itself is left to the next incremental update.
        assert_eq!(added, 50);
        let oldest: Version = fake_version(0).parse().unwrap();
        assert!(releases.iter().any(|release| release.version == oldest));
    }

    #[test]
    fn fetch_all_pages_stops_at_the_first_error() {
        let api = fake_api(1000);
        let result = fetch_all_now(
            |limit, offset| {
                if offset == 5 * FULL_PAGE_SIZE {
                    anyhow::bail!("page 5 failed");
                }
                api(limit, offset)
            },
            |_, _| {},
        );

        assert_eq!(result.unwrap_err().to_string(), "page 5 failed");
    }

    #[test]
    fn rate_limited_requests_are_retried() {
        let mut calls = 0;
        let result = retry_when_rate_limited(Duration::ZERO, || {
            calls += 1;
            if calls < 3 {
                Err(anyhow::Error::new(ureq::Error::StatusCode(429)).context("Failed to fetch"))
            } else {
                Ok(calls)
            }
        });

        assert_eq!(result.unwrap(), 3);
    }

    #[test]
    fn retries_give_up_and_other_errors_are_not_retried() {
        let mut calls = 0;
        let result: anyhow::Result<()> = retry_when_rate_limited(Duration::ZERO, || {
            calls += 1;
            Err(ureq::Error::StatusCode(429).into())
        });
        assert!(result.is_err());
        assert_eq!(calls, RATE_LIMIT_RETRIES + 1);

        let mut calls = 0;
        let result: anyhow::Result<()> = retry_when_rate_limited(Duration::ZERO, || {
            calls += 1;
            Err(ureq::Error::StatusCode(500).into())
        });
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }

    #[test]
    fn add_releases_skips_repeated_and_known_releases() {
        let mut releases = Releases::default();
        // A release published during a full download shifted the second page by one.
        let full_download = [
            release("2022.3.3f1", false),
            release("2022.3.2f1", true),
            release("2022.3.2f1", true),
            release("2022.3.1f1", false),
        ];

        assert_eq!(add_releases(&mut releases, full_download), 3);
        assert_eq!(
            releases.suggested_version.map(|v| v.to_string()),
            Some("2022.3.2f1".to_string())
        );

        let update = [release("2022.3.4f1", false), release("2022.3.3f1", false)];
        assert_eq!(add_releases(&mut releases, update), 1);
        assert_eq!(releases.releases.len(), 4);
    }
}
