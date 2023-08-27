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

pub struct HttpCache {
    cache_dir: PathBuf,
}

impl HttpCache {
    pub fn new() -> HttpCache {
        let cache_dir = cache_dir()
            .expect("unable to get cache directory")
            .join("ucom");
        HttpCache { cache_dir }
    }

    /// Gets the content of the given URL. Gets the content from the cache if it exists and is not too old.
    pub fn get(&self, url: &str) -> anyhow::Result<String> {
        if !HttpCache::is_cache_enabled() {
            let content = ureq::get(url).call()?.into_string()?;
            return Ok(content);
        }

        let filename = self.cache_dir.join(HttpCache::sanitize_filename(url));
        let path = Path::new(&filename);
        if path.exists() {
            let local_last_modified = metadata(&filename)?.modified()?;
            let local_last_modified = DateTime::<Utc>::from(local_last_modified);

            if Utc::now() - local_last_modified > Duration::hours(1) {
                self.download_and_save(url, &filename)
            } else {
                let content = fs::read_to_string(&filename)?;
                Ok(content)
            }
        } else {
            self.download_and_save(url, &filename)
        }
    }

    /// Clears the cache.
    #[allow(dead_code)]
    pub fn clear(&self) -> anyhow::Result<()> {
        _ = fs::remove_dir_all(&self.cache_dir);
        create_dir_all(&self.cache_dir)?;
        Ok(())
    }

    /// Sets whether the cache is enabled or not.
    #[allow(dead_code)]
    pub fn set_cache_enabled(enabled: bool) {
        let mut cache_enabled = CACHE_ENABLED.lock().unwrap();
        *cache_enabled = enabled;
    }

    /// Sets whether the cache is enabled or not based on environment variable UCOM_DISABLE_CACHE.
    pub fn set_cache_from_env() {
        if env::var("UCOM_DISABLE_CACHE").is_ok() {
            let mut cache_enabled = CACHE_ENABLED.lock().unwrap();
            *cache_enabled = false;
        }
    }

    /// Returns whether the cache is enabled or not.
    pub fn is_cache_enabled() -> bool {
        let cache_enabled = CACHE_ENABLED.lock().unwrap();
        *cache_enabled
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
    fn download_and_save(&self, url: &str, filename: &Path) -> anyhow::Result<String> {
        let content = ureq::get(url).call()?.into_string()?;
        create_dir_all(&self.cache_dir).expect("unable to create cache directory");
        fs::write(filename, &content)?;
        Ok(content)
    }
}
