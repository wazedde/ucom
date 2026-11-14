//! Reading build logs: the build report and error lines.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, anyhow};
use itertools::Itertools;

use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::MessageType;

pub(super) fn print_build_report(log_path: &Path, status: MessageType) {
    if let Ok(file) = File::open(log_path) {
        // Iterate over lines from the build report in the log file.
        log_lines(file)
            .skip_while(|l| !l.starts_with("[Builder] Build Report")) // Find marker.
            .skip(1) // Skip the marker.
            .take_while(|l| !l.is_empty()) // Read until empty line.
            .for_each(|l| {
                if let Some((key, value)) = l.split_once(':') {
                    MessageType::print_line(key.trim(), value.trim(), status);
                } else {
                    println!("{l}");
                }
            });
    }
}

/// Returns the lines of a log file.
///
/// Invalid UTF-8, which native plugins or the OS can write in a local code page, is replaced
/// instead of ending the iteration.
fn log_lines(file: File) -> impl Iterator<Item = String> {
    BufReader::new(file)
        .split(b'\n')
        .map_while(Result::ok)
        .map(|line| {
            let mut line = String::from_utf8(line)
                .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
            if line.ends_with('\r') {
                line.pop();
            }
            line
        })
}

/// Returns the unique error lines in the given log file.
pub(super) fn find_log_errors(log_file: &Path) -> std::io::Result<Vec<String>> {
    let file = File::open(log_file)?;
    Ok(log_lines(file)
        .filter(|l| line_contains_error(l))
        .unique()
        .collect_vec())
}

/// Returns errors from the given log file as one collected Err.
pub(super) fn collect_log_errors(log_file: &Path) -> anyhow::Error {
    match find_log_errors(log_file) {
        Ok(errors) => format_log_errors(&errors),
        Err(_) => anyhow!("Failed to open log file: {}", log_file.normalized_display()),
    }
}

/// Combines error lines from a log into one error.
pub(super) fn format_log_errors(errors: &[String]) -> anyhow::Error {
    match errors {
        [] => anyhow!("No errors found in log"),
        [single_error] => anyhow!("{single_error}"),
        _ => {
            let error_list = errors
                .iter()
                .enumerate()
                .map(|(i, error)| format!("{c}: {e}", c = i + 1, e = error))
                .join("\n");

            anyhow!(error_list)
        }
    }
}

/// Returns true if the given line is an error.
fn line_contains_error(line: &str) -> bool {
    const ERROR_PREFIXES: &[&str] = &[
        "[Builder] Error:",
        "error CS",
        "Fatal Error",
        "Error building Player",
        "error:",
        "BuildFailedException:",
        "System.Exception:",
    ];

    ERROR_PREFIXES.iter().any(|prefix| line.contains(prefix))
}

/// Removes the log file of a previous build, so a stale log is never reported.
pub(super) fn remove_old_log_file(log_path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(log_path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).with_context(|| {
            format!(
                "Failed to remove old log file: {}",
                log_path.normalized_display()
            )
        }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_util::TempDir;
    #[test]
    fn remove_old_log_file_removes_existing_and_ignores_missing() {
        let dir = TempDir::new("remove-old-log");
        let log_path = dir.path().join("Build-macos.log");

        remove_old_log_file(&log_path).unwrap();

        fs::write(&log_path, "old log").unwrap();
        remove_old_log_file(&log_path).unwrap();
        assert!(!log_path.exists());
    }

    #[test]
    fn find_log_errors_reads_past_invalid_utf8() {
        let dir = TempDir::new("log-invalid-utf8");
        let log_path = dir.path().join("Build.log");
        fs::write(
            &log_path,
            b"Plugin says \xE9t\xE9\r\nAssets/A.cs(1,1): error CS1002: ; expected\r\n",
        )
        .unwrap();

        let errors = find_log_errors(&log_path).unwrap();

        assert_eq!(errors, ["Assets/A.cs(1,1): error CS1002: ; expected"]);
    }
}
