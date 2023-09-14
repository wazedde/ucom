use std::env;
use std::fs;
use std::fs::{create_dir_all, metadata};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};
use dirs::cache_dir;
use lazy_static::lazy_static;

lazy_static! {
    static ref CACHE_ENABLED: Mutex<bool> = Mutex::new(true);
}

/// Gets the content of the given URL. Gets the content from the cache if it exists and is not too old.
pub fn get(url: &str) -> anyhow::Result<String> {
    if !is_cache_enabled() {
        let content = ureq::get(url).call()?.into_string()?;
        return Ok(content);
    }

    let cache_dir = ucom_cache_dir();
    let filename = cache_dir.join(sanitize_filename(url));
    let path = Path::new(&filename);

    if path.exists() {
        let local_last_modified = metadata(&filename)?.modified()?;
        let local_last_modified = DateTime::<Utc>::from(local_last_modified);

        if Utc::now() - local_last_modified > Duration::hours(1) {
            download_and_save(url, &filename, &cache_dir)
        } else {
            let content = fs::read_to_string(&filename)?;
            Ok(content)
        }
    } else {
        download_and_save(url, &filename, &cache_dir)
    }
}

/// Clears the cache.
pub fn clear() {
    _ = fs::remove_dir_all(ucom_cache_dir());
}

/// Sets whether the cache is enabled or not.
#[allow(dead_code)]
pub fn set_cache_enabled(enabled: bool) {
    let mut cache_enabled = CACHE_ENABLED.lock().unwrap();
    *cache_enabled = enabled;
}

/// Sets whether the cache is enabled or not based on environment variable `UCOM_ENABLE_CACHE`.
pub fn set_cache_from_env() {
    if let Ok(val) = env::var("UCOM_ENABLE_CACHE") {
        let mut cache_enabled = CACHE_ENABLED.lock().unwrap();
        *cache_enabled = val == "true" || val == "1";
    };
}

/// Returns whether the cache is enabled or not.
pub fn is_cache_enabled() -> bool {
    let cache_enabled = CACHE_ENABLED.lock().unwrap();
    *cache_enabled
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
fn download_and_save(url: &str, filename: &Path, cache_dir: &Path) -> anyhow::Result<String> {
    let content = ureq::get(url).call()?.into_string()?;
    create_dir_all(cache_dir).expect("unable to create cache directory");
    fs::write(filename, &content)?;
    Ok(content)
}
