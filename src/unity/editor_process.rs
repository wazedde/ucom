//! Detection of an interactive Unity editor that has a project open.

use crate::unity::ProjectPath;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
pub use sysinfo::Pid;
use sysinfo::{ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind};

/// Returns the process ID of an interactive Unity editor that has the given project open,
/// or `None` if there is none.
///
/// A process counts when it runs the Unity editor executable without `-batchmode`, and its
/// command line has `-projectPath` followed by a path that refers to the project. Unity Hub,
/// batch-mode processes (for example a running `ucom build` or `ucom test`, or the editor's
/// asset import workers) are ignored, since they cannot handle editor commands.
pub fn find_unity_editor_process(project: &ProjectPath) -> Option<Pid> {
    // First list processes with their names only, which is cheap, then read command lines
    // and working directories just for the processes running the Unity editor executable.
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );

    let candidates: Vec<Pid> = system
        .processes()
        .iter()
        .filter(|(_, process)| is_editor_executable(process.name()))
        .map(|(&pid, _)| pid)
        .collect();

    if candidates.is_empty() {
        return None;
    }

    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&candidates),
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::Always),
    );

    let project = comparable_path(project);
    let matches: Vec<(Pid, Option<Pid>)> = candidates
        .iter()
        .filter_map(|&pid| system.process(pid))
        .filter(|process| is_interactive_editor_for(process.cmd(), process.cwd(), &project))
        .map(|process| (process.pid(), process.parent()))
        .collect();

    select_top_level_process(&matches)
}

/// Returns true if the process with the given ID exists and has not terminated.
///
/// Only that one process is queried, and no command line is read, so the check does not
/// depend on access to other processes' details.
pub fn is_process_running(pid: Pid) -> bool {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );

    system
        .process(pid)
        .is_some_and(|process| process.status() != ProcessStatus::Zombie)
}

/// Returns true if a process name is that of the Unity editor executable
/// (`Unity` on macOS and Linux, `Unity.exe` on Windows).
fn is_editor_executable(name: &OsStr) -> bool {
    name.eq_ignore_ascii_case("Unity") || name.eq_ignore_ascii_case("Unity.exe")
}

/// Returns true if `cmd` is the command line of an interactive (non-batch) Unity editor
/// that has the project at `project` (as returned by [`comparable_path`]) open.
///
/// A relative `-projectPath` value is resolved against the process's working directory
/// `cwd`; if that is unknown, the value cannot be resolved and does not match.
fn is_interactive_editor_for(cmd: &[OsString], cwd: Option<&Path>, project: &Path) -> bool {
    // Batch-mode processes do not run the editor command watcher.
    if cmd.iter().any(|arg| arg.eq_ignore_ascii_case("-batchmode")) {
        return false;
    }

    cmd.windows(2).any(|pair| {
        if !pair[0].eq_ignore_ascii_case("-projectPath") {
            return false;
        }

        let value = Path::new(&pair[1]);
        let resolved = if value.is_absolute() {
            value.to_path_buf()
        } else if let Some(cwd) = cwd {
            cwd.join(value)
        } else {
            return false;
        };

        comparable_path(&resolved) == project
    })
}

/// Picks the process to track from the matching `(pid, parent)` pairs: one whose parent is
/// not itself a match, so a matching child process is never chosen over its parent.
fn select_top_level_process(matches: &[(Pid, Option<Pid>)]) -> Option<Pid> {
    let pids: HashSet<Pid> = matches.iter().map(|&(pid, _)| pid).collect();

    matches
        .iter()
        .find(|(_, parent)| parent.is_none_or(|parent| !pids.contains(&parent)))
        .map(|&(pid, _)| pid)
}

/// Returns a form of `path` that can be compared for equality. Symbolic links and relative
/// components are resolved, and on case-insensitive file systems the spelling stored on disk
/// is used, so paths that differ only in case compare equal there but not on case-sensitive
/// volumes.
pub fn comparable_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_util::TempDir;

    fn cmd(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    fn pid(n: u32) -> Pid {
        Pid::from_u32(n)
    }

    #[test]
    fn editor_executable_names() {
        assert!(is_editor_executable(OsStr::new("Unity")));
        assert!(is_editor_executable(OsStr::new("Unity.exe")));
        assert!(is_editor_executable(OsStr::new("unity")));
        assert!(!is_editor_executable(OsStr::new("Unity Hub")));
        assert!(!is_editor_executable(OsStr::new("Unity Hub.exe")));
        assert!(!is_editor_executable(OsStr::new("unityhub")));
        assert!(!is_editor_executable(OsStr::new("UnityShaderCompiler")));
    }

    #[test]
    fn editor_detection_matches_only_the_exact_project() {
        let temp = TempDir::new("detect");
        let root = temp.path().to_path_buf();
        let game = root.join("Game");
        let game2 = root.join("Game2");
        let other = root.join("Other");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&game2).unwrap();
        fs::create_dir_all(other.join("Game")).unwrap();
        let project = comparable_path(&game);
        let editor = "/Applications/Unity/Hub/Editor/6000.0.1f1/Unity.app/Contents/MacOS/Unity";

        let game_str = game.to_str().unwrap();
        let game2_str = game2.to_str().unwrap();
        let matches = |args: &[&str], cwd: Option<&Path>| {
            is_interactive_editor_for(&cmd(args), cwd, &project)
        };

        // Exact project, case-insensitive flag, trailing separator.
        assert!(matches(&[editor, "-projectPath", game_str], None));
        assert!(matches(&[editor, "-projectpath", game_str], None));
        let with_slash = format!("{game_str}{}", std::path::MAIN_SEPARATOR);
        assert!(matches(&[editor, "-projectPath", &with_slash], None));

        // A project whose path merely starts with the same text.
        assert!(!matches(&[editor, "-projectPath", game2_str], None));

        // The path must be the value of `-projectPath`, not just appear somewhere.
        assert!(!matches(
            &[editor, "-logFile", game_str, "-projectPath"],
            None
        ));

        // Batch-mode processes are not interactive editors (any flag case).
        assert!(!matches(
            &[editor, "-batchmode", "-projectPath", game_str],
            None
        ));
        assert!(!matches(
            &[editor, "-batchMode", "-projectPath", game_str],
            None
        ));

        // Relative paths resolve against the editor process's working directory.
        assert!(matches(&[editor, "-projectPath", "Game"], Some(&root)));
        assert!(!matches(&[editor, "-projectPath", "Game"], Some(&other)));
        assert!(!matches(&[editor, "-projectPath", "Game"], None));

        // Case-insensitive file systems (the default on macOS and Windows) resolve to the
        // on-disk spelling.
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            let upper = game_str.to_uppercase();
            assert!(matches(&[editor, "-projectPath", &upper], None));
        }
    }

    #[test]
    fn top_level_process_is_preferred() {
        assert_eq!(select_top_level_process(&[]), None);
        assert_eq!(
            select_top_level_process(&[(pid(10), Some(pid(1)))]),
            Some(pid(10))
        );
        // A child (20) of a matching parent (30) is skipped, even though it comes first.
        assert_eq!(
            select_top_level_process(&[(pid(20), Some(pid(30))), (pid(30), Some(pid(1)))]),
            Some(pid(30))
        );
    }

    #[test]
    fn process_running_check() {
        assert!(is_process_running(Pid::from_u32(std::process::id())));

        let mut child = std::process::Command::new(if cfg!(windows) { "cmd" } else { "true" })
            .args(if cfg!(windows) {
                &["/C", "exit"][..]
            } else {
                &[][..]
            })
            .spawn()
            .unwrap();
        let pid = Pid::from_u32(child.id());
        child.wait().unwrap();
        assert!(!is_process_running(pid));
    }
}
