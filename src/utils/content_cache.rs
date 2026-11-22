use std::fmt::Display;
use std::fs;
use std::fs::{File, create_dir_all};
use std::io::{BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::{warn_non_fatal, write_atomic_with};
use anyhow::{Context, anyhow};
use chrono::{DateTime, TimeDelta, Utc};
use dirs::cache_dir;

const CACHE_REFRESH_SECONDS: i64 = 3600;

enum CacheState {
    /// Cache is outdated and must be refetched from source.
    Stale,
    /// Cache is current and can be used directly.
    Fresh,
    /// Cache is usable but should have its timestamp updated.
    Touched,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum RemoteChangeCheck {
    /// Check if the remote content is newer than the local file.
    Validate,
    /// Do not check for remote changes.
    Skip,
}

/// Gets the content of the given URL. Gets the content from the cache if it exists and is not too old.
pub fn fetch_content(url: &str, change_check: RemoteChangeCheck) -> anyhow::Result<String> {
    let cache_dir = ucom_cache_dir()?;
    let filename = cache_dir.join(sanitize_filename(url));

    match determine_cache_status(url, &filename, change_check) {
        CacheState::Stale => match fetch_and_store_in_cache(url, &filename) {
            // When the request fails, for example offline, an older copy is used without a
            // warning: the content (release notes, templates) rarely changes, and a warning
            // would garble a status line that a caller may be showing.
            Err(e) if e.downcast_ref::<ureq::Error>().is_some() && filename.exists() => {}
            result => return result,
        },
        CacheState::Fresh => {}
        CacheState::Touched => {
            // A failed timestamp update only means the remote check is repeated next time.
            let _ = touch_file(&filename);
        }
    }

    fs::read_to_string(&filename).with_context(|| {
        format!(
            "Failed to read cached file: {}",
            filename.normalized_display()
        )
    })
}

/// Writes a cache file atomically, creating the cache directory if needed.
///
/// Caching is best-effort: if the file cannot be written, a warning is printed and the
/// caller continues with the data it already has.
pub fn store_in_cache(path: &Path, write: impl FnOnce(&mut BufWriter<File>) -> anyhow::Result<()>) {
    let store = || -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            create_dir_all(dir)?;
        }
        write_atomic_with(path, write)
    };

    if let Err(e) = store() {
        warn_non_fatal!(
            "Could not write to the cache at {}: {e:#}",
            path.normalized_display()
        );
    }
}

/// Deletes the cache directory. A cache directory that does not exist counts as deleted.
pub fn delete_cache_directory(dir: &Path) -> anyhow::Result<()> {
    match fs::remove_dir_all(dir) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e).with_context(|| {
            format!(
                "Cannot remove the cache directory: {}",
                dir.normalized_display()
            )
        }),
        _ => Ok(()),
    }
}

/// Returns the cache directory for ucom.
pub fn ucom_cache_dir() -> anyhow::Result<PathBuf> {
    cache_dir()
        .map(|p| p.join("ucom"))
        .ok_or_else(|| anyhow!("Unable to get a cache directory"))
}

/// Returns whether the given file is too old to be considered valid.
pub fn is_cached_file_stale(path: &Path) -> bool {
    // A file whose modification time cannot be read is stale.
    !modified_time(path).is_ok_and(is_fresh)
}

/// Returns whether a cache file modified at `modified` is recent enough to use without checking
/// the source. A modification time in the future is not fresh.
fn is_fresh(modified: SystemTime) -> bool {
    let age = Utc::now() - DateTime::<Utc>::from(modified);
    age >= TimeDelta::zero() && age <= TimeDelta::seconds(CACHE_REFRESH_SECONDS)
}

pub fn modified_time(path: &Path) -> std::io::Result<SystemTime> {
    path.metadata()?.modified()
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
                "Failed to update the timestamp on {}",
                path.normalized_display()
            )
        })
}

#[cfg(windows)]
fn is_timestamp_permission_error(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::PermissionDenied
}

#[cfg(not(windows))]
fn is_timestamp_permission_error(_err: &std::io::Error) -> bool {
    false
}

/// Checks if the cached content is up to date.
///
/// A missing cache file, or one whose modification time cannot be read, is stale.
fn determine_cache_status(url: &str, cached: &Path, change_check: RemoteChangeCheck) -> CacheState {
    let Ok(cached_time) = modified_time(cached) else {
        return CacheState::Stale;
    };

    if is_fresh(cached_time) {
        CacheState::Fresh
    } else if change_check == RemoteChangeCheck::Validate
        && !is_remote_content_newer(url, cached_time).unwrap_or(true)
    {
        // The local file is newer than the remote Last-Modified time.
        CacheState::Touched
    } else {
        CacheState::Stale
    }
}

/// Checks if the remote page has been updated since the given time.
fn is_remote_content_newer(url: &str, local_time: SystemTime) -> anyhow::Result<bool> {
    let local_datetime = DateTime::<Utc>::from(local_time);
    let remote_datetime =
        fetch_last_modified_time(url).context("Failed to determine if remote content is newer")?;

    Ok(local_datetime < remote_datetime)
}

fn sanitize_filename(filename: &str) -> String {
    filename.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
}

/// Returns the HTTP agent for all of ucom's requests.
///
/// The timeouts keep an unresponsive server from blocking ucom forever. They are generous,
/// because the Unity release API can be slow.
pub fn http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .timeout_recv_body(Some(Duration::from_secs(120)))
            .build()
            .into()
    })
}

/// Returns an error for a failed request that says what went wrong in plain words: no
/// connection, no answer in time, or an HTTP status. The `ureq` error stays in the chain as the
/// cause, so callers can still downcast to it.
pub fn request_error(error: ureq::Error, action: impl Display, url: &str) -> anyhow::Error {
    let uri = url.parse::<ureq::http::Uri>().ok();
    let host = uri.as_ref().and_then(|uri| uri.host()).unwrap_or(url);
    let reason = match &error {
        // These happen after the connection was made, so the network itself is not the problem.
        ureq::Error::Io(e)
            if matches!(
                e.kind(),
                ErrorKind::ConnectionReset
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::BrokenPipe
                    | ErrorKind::UnexpectedEof
            ) =>
        {
            format!("the connection to `{host}` was interrupted")
        }
        ureq::Error::Io(_) | ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => {
            format!("cannot connect to `{host}`; check the internet connection")
        }
        ureq::Error::Timeout(_) => format!("`{host}` did not answer in time"),
        ureq::Error::StatusCode(code) => format!("`{host}` answered with HTTP status {code}"),
        _ => format!("the request to `{host}` failed"),
    };
    anyhow::Error::new(error).context(format!("{action}: {reason}"))
}

/// Downloads the content from the given URL and saves it to the given filename.
///
/// Storing the content in the cache is best-effort: if the cache cannot be written,
/// a warning is printed and the downloaded content is still returned.
fn fetch_and_store_in_cache(url: &str, filename: &Path) -> anyhow::Result<String> {
    let content = http_agent()
        .get(url)
        .call()
        .map_err(|e| request_error(e, format_args!("Cannot download {url}"), url))?
        .into_body()
        .read_to_string()
        .context("Failed to read response body")?;

    store_in_cache(filename, |writer| Ok(writer.write_all(content.as_bytes())?));
    Ok(content)
}

fn fetch_last_modified_time(url: &str) -> anyhow::Result<DateTime<Utc>> {
    let response = http_agent()
        .head(url)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_errors_name_the_host_and_the_reason() {
        let url = "https://example.com/path?q=1";
        for (error, message) in [
            (
                ureq::Error::HostNotFound,
                "Cannot download: cannot connect to `example.com`; check the internet connection",
            ),
            (
                ureq::Error::Io(ErrorKind::ConnectionReset.into()),
                "Cannot download: the connection to `example.com` was interrupted",
            ),
            (
                ureq::Error::StatusCode(503),
                "Cannot download: `example.com` answered with HTTP status 503",
            ),
        ] {
            assert_eq!(
                request_error(error, "Cannot download", url).to_string(),
                message
            );
        }
    }
}
