use std::env;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use crate::utils::path_ext::PlatformConsistentPathExt;
use anyhow::{Context, anyhow};
use chrono::{DateTime, Duration, TimeDelta, Utc};
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

#[derive(Eq, PartialEq)]
pub enum RemoteChangeCheck {
    /// Check if the remote content is newer than the local file.
    Validate,
    /// Do not check for remote changes.
    Skip,
}

/// Gets the content of the given URL. Gets the content from the cache if it exists and is not too old.
pub fn fetch_content(url: &str, change_check: RemoteChangeCheck) -> anyhow::Result<String> {
    if !is_cache_enabled() {
        return ureq::get(url)
            .call()
            .with_context(|| format!("Failed to fetch {url}"))?
            .into_body()
            .read_to_string()
            .map_err(anyhow::Error::msg);
    }

    let cache_dir = ucom_cache_dir()?;
    let filename = cache_dir.join(sanitize_filename(url));

    match determine_cache_status(url, &filename, change_check)? {
        CacheState::Expired => fetch_and_store_in_cache(url, &filename, &cache_dir),
        CacheState::Valid => fs::read_to_string(&filename).map_err(anyhow::Error::msg),
        CacheState::RefreshNeeded => refresh_file_timestamp_and_read(&filename),
    }
}

/// Deletes the cache directory.
pub fn delete_cache_directory() {
    if let Ok(dir) = ucom_cache_dir() {
        fs::remove_dir_all(dir).ok();
    }
}

/// Sets whether the cache is enabled or not based on environment variable `UCOM_ENABLE_CACHE`.
pub fn configure_cache_from_environment() -> anyhow::Result<()> {
    match env::var("UCOM_ENABLE_CACHE") {
        Ok(val) => {
            let enabled = val == "true" || val == "1";
            CACHE_ENABLED
                .set(enabled)
                .map_err(|_| anyhow!("Failed to set CACHE_ENABLED"))
        }
        Err(_) => Ok(()), // environment variable not set
    }
}

/// Returns whether the cache is enabled or not.
pub fn is_cache_enabled() -> bool {
    *CACHE_ENABLED.get().unwrap_or(&true)
}

/// Returns the cache directory for ucom.
pub fn ucom_cache_dir() -> anyhow::Result<PathBuf> {
    cache_dir()
        .map(|p| p.join("ucom"))
        .ok_or_else(|| anyhow!("Unable to get cache directory"))
}

/// Returns whether the cached file is expired.
pub fn is_cache_file_expired(path: &Path) -> bool {
    let cached_time: DateTime<Utc> = match path.metadata().and_then(|m| m.modified()) {
        Ok(modified) => DateTime::<Utc>::from(modified),
        Err(_) => return true, // Cannot get modification time; consider expired
    };

    let delta_time = Utc::now() - cached_time;

    if delta_time < Duration::zero() {
        return true; // Modification time is in the future; consider expired
    }

    delta_time > Duration::seconds(CACHE_REFRESH_SECONDS)
}

/// Touches the timestamp of the given file.
pub fn touch_file(path: &Path) -> anyhow::Result<()> {
    fs::File::open(path)?
        .set_modified(SystemTime::now())
        .or_else(|e| {
            if is_timestamp_permission_error(&e) {
                // Platform-specific workaround
                let content = fs::read_to_string(path)?;
                fs::write(path, &content)?;
                Ok(())
            } else {
                Err(e)
            }
        })
        .with_context(|| {
            format!(
                "Failed to update timestamp on {}",
                path.normalized_display()
            )
        })
}

#[cfg(windows)]
fn is_timestamp_permission_error(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::PermissionDenied
}

#[cfg(not(windows))]
fn is_timestamp_permission_error(_err: &std::io::Error) -> bool {
    false
}

/// Checks if the cached content is up-to-date.
fn determine_cache_status(
    url: &str,
    cached: &Path,
    change_check: RemoteChangeCheck,
) -> anyhow::Result<CacheState> {
    let state = if cached.exists() {
        let cached_time = cached.metadata()?.modified()?;
        let delta_time = Utc::now() - DateTime::<Utc>::from(cached_time);

        if delta_time <= TimeDelta::try_seconds(CACHE_REFRESH_SECONDS).unwrap_or(TimeDelta::zero())
        {
            // Local file is still fresh enough
            CacheState::Valid
        } else if change_check == RemoteChangeCheck::Validate
            && !is_remote_content_newer(url, cached_time).unwrap_or(true)
        {
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

/// Checks if the remote page has been updated since the given time.
fn is_remote_content_newer(url: &str, local_time: SystemTime) -> anyhow::Result<bool> {
    let local_datetime = DateTime::<Utc>::from(local_time);
    let remote_datetime = fetch_last_modified_time(url)
        .with_context(|| "Failed to determine if remote content is newer")?;

    Ok(local_datetime < remote_datetime)
}

fn sanitize_filename(filename: &str) -> String {
    filename.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
}

/// Downloads the content from the given URL and saves it to the given filename.
fn fetch_and_store_in_cache(
    url: &str,
    filename: &Path,
    cache_dir: &Path,
) -> anyhow::Result<String> {
    let content = ureq::get(url).call()?.into_body().read_to_string()?;
    create_dir_all(cache_dir).context("Failed to create cache directory")?;
    fs::write(filename, &content)?;
    Ok(content)
}

fn refresh_file_timestamp_and_read(filename: &Path) -> anyhow::Result<String> {
    const ERROR_ACCESS_DENIED: i32 = 5;

    // Update the local timestamp
    match fs::File::open(filename)?.set_modified(Utc::now().into()) {
        Ok(()) => fs::read_to_string(filename).map_err(anyhow::Error::msg),
        Err(e) if e.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
            // If error is a permission error, do workaround by re-saving the file
            let content = fs::read_to_string(filename)?;
            fs::write(filename, &content)?;
            Ok(content)
        }
        Err(e) => Err(e.into()),
    }
}

fn fetch_last_modified_time(url: &str) -> anyhow::Result<DateTime<Utc>> {
    let response = ureq::head(url)
        .call()
        .with_context(|| format!("Failed to fetch Last-Modified header from {url}"))?;

    let last_modified_header = response
        .headers()
        .get("Last-Modified")
        .and_then(|s| s.to_str().ok())
        .ok_or_else(|| anyhow!("Last-Modified header not found in response from {url}"))?;

    DateTime::parse_from_rfc2822(last_modified_header)
        .map(|t| t.with_timezone(&Utc))
        .with_context(|| format!("Failed to parse Last-Modified header from {url}"))
}
