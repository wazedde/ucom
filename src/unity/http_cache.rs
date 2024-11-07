use std::env;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use anyhow::anyhow;
use chrono::{DateTime, TimeDelta, Utc};
use dirs::cache_dir;

static CACHE_ENABLED: OnceLock<bool> = OnceLock::new();

const CACHE_REFRESH_SECONDS: i64 = 3600;

enum CacheState {
    /// The cache is expired.
    Expired,
    /// The cache is still valid.
    Valid,
    /// The cache is still valid but needs to be refreshed.
    RefreshNeeded,
}

/// Gets the content of the given URL. Gets the content from the cache if it exists and is not too old.
pub(crate) fn fetch_content(url: &str, check_for_remote_change: bool) -> anyhow::Result<String> {
    if !is_cache_enabled() {
        return Ok(ureq::get(url).call()?.into_string()?);
    }

    let cache_dir = ucom_cache_dir();
    let filename = cache_dir.join(sanitize_filename(url));

    match get_cache_state(url, &filename, check_for_remote_change)? {
        CacheState::Expired => fetch_and_save_to_cache(url, &filename, &cache_dir),
        CacheState::Valid => fs::read_to_string(&filename).map_err(anyhow::Error::msg),
        CacheState::RefreshNeeded => read_and_refresh(&filename),
    }
}

/// Clears the cache.
pub(crate) fn clear() {
    _ = fs::remove_dir_all(ucom_cache_dir());
}

/// Sets whether the cache is enabled or not.
/// This value can only be set once.
pub(crate) fn set_cache_enabled(enabled: bool) -> anyhow::Result<()> {
    CACHE_ENABLED
        .set(enabled)
        .map_err(|_| anyhow!("Failed to set CACHE_ENABLED"))
}

/// Sets whether the cache is enabled or not based on environment variable `UCOM_ENABLE_CACHE`.
pub(crate) fn enable_cache_from_env() -> anyhow::Result<()> {
    match env::var("UCOM_ENABLE_CACHE") {
        Ok(val) => set_cache_enabled(val == "true" || val == "1"),
        Err(_) => Ok(()),
    }
}

/// Returns whether the cache is enabled or not.
pub(crate) fn is_cache_enabled() -> bool {
    *CACHE_ENABLED.get().unwrap_or(&true)
}

pub(crate) fn ucom_cache_dir() -> PathBuf {
    cache_dir()
        .expect("unable to get cache directory")
        .join("ucom")
}

/// Checks if the cached content is up-to-date.
fn get_cache_state(
    url: &str,
    cached: &Path,
    check_for_remote_change: bool,
) -> anyhow::Result<CacheState> {
    let state = if cached.exists() {
        let cached_time = cached.metadata()?.modified()?;
        let delta_time = Utc::now() - DateTime::<Utc>::from(cached_time);

        if delta_time <= TimeDelta::try_seconds(CACHE_REFRESH_SECONDS).unwrap() {
            // Local file is still new enough
            CacheState::Valid
        } else if check_for_remote_change && !is_remote_newer_than_local(url, &cached_time) {
            // Local file is newer than remote Last-Modified
            CacheState::RefreshNeeded
        } else {
            // Local file is out of date
            CacheState::Expired
        }
    } else {
        // Has no cache file
        CacheState::Expired
    };
    Ok(state)
}

pub(crate) fn has_expired(path: &Path) -> bool {
    let cached_time = path
        .metadata()
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let delta_time = Utc::now() - DateTime::<Utc>::from(cached_time);
    delta_time > TimeDelta::seconds(CACHE_REFRESH_SECONDS)
}

pub(crate) fn touch_timestamp(filename: &PathBuf) -> anyhow::Result<()> {
    // Update the local timestamp
    match fs::File::open(filename)?.set_modified(Utc::now().into()) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(5) => {
            // If error is a permission error, do workaround by re-saving the file
            let content = fs::read_to_string(filename)?;
            fs::write(filename, &content)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
/// Checks if the page has been updated since the given time.
fn is_remote_newer_than_local(url: &str, local_time: &SystemTime) -> bool {
    if let Ok(server_utc) = fetch_remote_last_modified_time(url) {
        let local_utc = DateTime::<Utc>::from(*local_time);
        local_utc < server_utc
    } else {
        // Always update if we couldn't check
        true
    }
}

fn sanitize_filename(filename: &str) -> String {
    filename.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
}

/// Downloads the content from the given URL and saves it to the given filename.
fn fetch_and_save_to_cache(url: &str, filename: &Path, cache_dir: &Path) -> anyhow::Result<String> {
    let content = ureq::get(url).call()?.into_string()?;
    create_dir_all(cache_dir).expect("unable to create cache directory");
    fs::write(filename, &content)?;
    Ok(content)
}

fn read_and_refresh(filename: &PathBuf) -> anyhow::Result<String> {
    // Update the local timestamp
    match fs::File::open(filename)?.set_modified(Utc::now().into()) {
        Ok(()) => fs::read_to_string(filename).map_err(anyhow::Error::msg),
        Err(e) if e.raw_os_error() == Some(5) => {
            // If error is a permission error, do workaround by re-saving the file
            let content = fs::read_to_string(filename)?;
            fs::write(filename, &content)?;
            Ok(content)
        }
        Err(e) => Err(e.into()),
    }
}

fn fetch_remote_last_modified_time(url: &str) -> anyhow::Result<DateTime<Utc>> {
    let response = ureq::head(url).call()?;
    let lm = response
        .header("Last-Modified")
        .ok_or_else(|| anyhow!("Last-Modified header not found in response from {}", url))?;
    let time = DateTime::parse_from_rfc2822(lm)?.with_timezone(&Utc);
    Ok(time)
}
