use std::env;
use std::fs;
use std::fs::{create_dir_all, metadata};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::anyhow;
use chrono::{DateTime, TimeDelta, Utc};
use dirs::cache_dir;

static CACHE_ENABLED: OnceLock<bool> = OnceLock::new();

/// Gets the content of the given URL. Gets the content from the cache if it exists and is not too old.
pub fn fetch_content(url: &str) -> anyhow::Result<String> {
    if !is_cache_enabled() {
        return Ok(ureq::get(url).call()?.into_string()?);
    }

    let cache_dir = ucom_cache_dir();
    let filename = cache_dir.join(sanitize_filename(url));

    if Path::new(&filename).exists() {
        let modified_time = metadata(&filename)?.modified()?;
        let delta_time = Utc::now() - DateTime::<Utc>::from(modified_time);
        if delta_time <= TimeDelta::try_hours(1).unwrap() {
            return Ok(fs::read_to_string(&filename)?);
        }
    }
    fetch_and_save(url, &filename, &cache_dir)
}

/// Clears the cache.
pub fn clear() {
    _ = fs::remove_dir_all(ucom_cache_dir());
}

/// Sets whether the cache is enabled or not.
/// This value can only be set once.
pub fn set_cache_enabled(enabled: bool) -> anyhow::Result<()> {
    CACHE_ENABLED
        .set(enabled)
        .map_err(|_| anyhow!("Failed to set CACHE_ENABLED"))
}

/// Sets whether the cache is enabled or not based on environment variable `UCOM_ENABLE_CACHE`.
pub fn set_cache_from_env() -> anyhow::Result<()> {
    if let Ok(val) = env::var("UCOM_ENABLE_CACHE") {
        set_cache_enabled(val == "true" || val == "1")
    } else {
        Ok(())
    }
}

/// Returns whether the cache is enabled or not.
pub fn is_cache_enabled() -> bool {
    *CACHE_ENABLED.get().unwrap_or(&true)
}

pub fn ucom_cache_dir() -> PathBuf {
    cache_dir()
        .expect("unable to get cache directory")
        .join("ucom")
}

fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| match c {
            '.' | ' ' | '\\' | '/' | ':' | '*' | '?' | '\"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Downloads the content from the given URL and saves it to the given filename.
fn fetch_and_save(url: &str, filename: &Path, cache_dir: &Path) -> anyhow::Result<String> {
    let content = ureq::get(url).call()?.into_string()?;
    create_dir_all(cache_dir).expect("unable to create cache directory");
    fs::write(filename, &content)?;
    Ok(content)
}
