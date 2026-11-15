use crate::utils::path_ext::PlatformConsistentPathExt;
use anyhow::{Context, ensure};
use path_absolutize::Absolutize;
use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

pub mod cancel;
pub mod content_cache;
pub mod formatter;
pub mod native_progress;
pub mod path_ext;
pub mod report;
pub mod spawn_cmd;
pub mod status_line;
pub mod vec1;

#[cfg(test)]
pub mod test_util;

/// Prints a non-fatal warning to stderr.
macro_rules! warn_non_fatal {
    ($($arg:tt)*) => {
        eprintln!("Warning: {}", format_args!($($arg)*));
    };
}

pub(crate) use warn_non_fatal;

/// An error that only decides how ucom ends, without the usual `Error:` output.
#[derive(Debug)]
pub enum ReportedFailure {
    /// The command has already reported the failure; `main` exits with `exit_code`.
    Silent {
        exit_code: u8,
        description: &'static str,
    },
    /// The user pressed Ctrl+C; `main` prints a short notice and ends ucom as Ctrl+C would.
    Interrupted,
}

impl ReportedFailure {
    /// Returns the reported failure in the error's chain, if any.
    pub fn find(error: &anyhow::Error) -> Option<&Self> {
        error.chain().find_map(|e| e.downcast_ref::<Self>())
    }
}

impl std::fmt::Display for ReportedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Silent { description, .. } => f.write_str(description),
            Self::Interrupted => f.write_str("Cancelled"),
        }
    }
}

impl std::error::Error for ReportedFailure {}

/// Writes `contents` to `path` so that readers never observe a partially written file.
///
/// The data is first written to a uniquely named sibling file ending in `.tmp`, which is
/// then renamed over `path`. A rename within the same directory is atomic on all supported
/// platforms, and replaces an existing destination file. The unique name keeps concurrent
/// writers of the same `path` from sharing a temporary file; the last rename wins.
pub fn write_atomic(path: &Path, contents: impl AsRef<[u8]>) -> anyhow::Result<()> {
    write_atomic_with(path, |writer| Ok(writer.write_all(contents.as_ref())?))
}

/// Like [`write_atomic`], but streams the content through a buffered writer instead of
/// requiring it in memory up front.
pub fn write_atomic_with(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let tmp_path = unique_temp_path(path)?;

    let result = write_and_rename(&tmp_path, path, write)
        .with_context(|| format!("Failed to write file: {}", path.normalized_display()));

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

/// Returns a uniquely named sibling of `path` ending in `.tmp`.
fn unique_temp_path(path: &Path) -> anyhow::Result<PathBuf> {
    let file_name = path
        .file_name()
        .with_context(|| format!("Invalid file path: {}", path.normalized_display()))?;
    let mut tmp_name = file_name.to_owned();
    tmp_name.push(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    Ok(path.with_file_name(tmp_name))
}

fn write_and_rename(
    tmp_path: &Path,
    path: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut writer = BufWriter::new(File::create(tmp_path)?);
    write(&mut writer)?;
    // Flush explicitly: errors from flushing on drop would be lost.
    writer
        .into_inner()
        .map_err(std::io::IntoInnerError::into_error)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

/// Returns the absolute path to an existing directory.
pub fn resolve_absolute_dir_path(path: &impl AsRef<Path>) -> anyhow::Result<Cow<'_, Path>> {
    let path = path.as_ref();

    ensure!(
        !(cfg!(target_os = "windows") && path.starts_with("~")),
        "On Windows the path cannot start with '~': `{}`",
        path.normalized_display()
    );

    ensure!(
        path.is_dir(),
        "Path does not exist or is not a directory: `{}`",
        path.normalized_display()
    );

    let path = path.absolutize().context("Failed to absolutize the path")?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_util::TempDir;

    #[test]
    fn write_atomic_replaces_file_and_leaves_no_temp_file() {
        let temp = TempDir::new("write-atomic");
        let dir = temp.path();
        let path = dir.join("build-test.json");

        write_atomic(&path, "first").unwrap();
        write_atomic(&path, "second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        let entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
        assert_eq!(entries.len(), 1, "temporary files left behind: {entries:?}");
    }

    #[test]
    fn write_atomic_concurrent_writers_produce_a_complete_file() {
        let temp = TempDir::new("write-atomic");
        let dir = temp.path();
        let path = dir.join("releases.json");

        let payloads: Vec<String> = (b'a'..=b'h')
            .map(|c| String::from(c as char).repeat(256 * 1024))
            .collect();

        std::thread::scope(|scope| {
            for payload in &payloads {
                let path = &path;
                scope.spawn(move || {
                    for _ in 0..5 {
                        write_atomic(path, payload).unwrap();
                    }
                });
            }
        });

        // The result must be exactly one writer's payload, never a mix.
        let content = fs::read_to_string(&path).unwrap();
        assert!(
            payloads.contains(&content),
            "file content is a mix of writers"
        );
        let entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
        assert_eq!(entries.len(), 1, "temporary files left behind: {entries:?}");
    }
}
