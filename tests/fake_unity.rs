//! End-to-end tests that run the real `ucom` binary against a fake Unity.
//!
//! This test program is also the fake: each test copies it into a temporary folder under the
//! Unity executable name, and `ucom` (or the test) starts it with `UCOM_FAKE_UNITY` set to the
//! behaviour wanted. Without that variable it runs the tests.
//!
//! The fake covers what ucom sees of Unity: the process name and arguments (editor detection),
//! the log file and exit code (batch builds), the command and result files (editor builds), and
//! how it reacts to Ctrl+C. It does not cover `UnityBuilder.cs` or real Unity behaviour.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Output, Stdio};
use std::time::{Duration, Instant};
use std::{env, fs, panic, thread};

/// Selects the fake's behaviour; see [`fake::run`].
const FAKE_MODE: &str = "UCOM_FAKE_UNITY";

/// The Unity version of the test projects. It must be supported by the builder scripts.
const UNITY_VERSION: &str = "6000.0.1f1";

fn main() -> ExitCode {
    match env::var(FAKE_MODE) {
        Ok(mode) => fake::run(&mode),
        Err(_) => harness::run(TESTS),
    }
}

/// The tests, run in order.
const TESTS: &[(&str, fn())] = &[
    (
        "batch_build_succeeds_and_removes_the_injected_script",
        batch_build_succeeds_and_removes_the_injected_script,
    ),
    (
        "editor_build_succeeds_and_moves_the_log",
        editor_build_succeeds_and_moves_the_log,
    ),
    #[cfg(unix)]
    (
        "cancelled_batch_build_removes_the_injected_script",
        unix::cancelled_batch_build_removes_the_injected_script,
    ),
    #[cfg(unix)]
    (
        "second_ctrl_c_kills_a_unity_that_ignores_it",
        unix::second_ctrl_c_kills_a_unity_that_ignores_it,
    ),
    #[cfg(unix)]
    (
        "cancelled_editor_build_removes_the_command_file",
        unix::cancelled_editor_build_removes_the_command_file,
    ),
];

//
// Tests
//

fn batch_build_succeeds_and_removes_the_injected_script() {
    let env = TestEnv::new("batch-ok");
    env.install_fake_editor();

    let output = env
        .ucom(&[
            "build",
            "webgl",
            &env.project_arg(),
            "-o",
            &env.path_arg("out"),
            "-l",
            &env.path_arg("build.log"),
        ])
        .env(FAKE_MODE, "batch")
        .output()
        .unwrap();

    assert_success(&output);
    assert_contains(&stdout(&output), "Succeeded building");
    // The fake writes its arguments to the log, which ucom must have kept at the -l path.
    let log = fs::read_to_string(env.root.join("build.log")).unwrap();
    assert_contains(&log, "-batchmode");
    assert_contains(&log, "UnityBuilder.Build");
    assert!(
        env.injected_scripts().is_empty(),
        "injected script left behind"
    );
}

fn editor_build_succeeds_and_moves_the_log() {
    let env = TestEnv::new("editor-ok");
    env.add_builder();
    let editor = env.start_fake_editor("editor");

    let output = env
        .ucom(&[
            "build",
            "webgl",
            &env.project_arg(),
            "-o",
            &env.path_arg("out"),
            "-l",
            &env.path_arg("build.log"),
        ])
        .output()
        .unwrap();
    drop(editor);

    assert_success(&output);
    assert_contains(&stdout(&output), "Succeeded building");
    assert_contains(&stdout(&output), "via editor");
    let log = fs::read_to_string(env.root.join("build.log")).unwrap();
    assert_contains(&log, "fake editor build");
    assert!(
        env.ipc_files().is_empty(),
        "IPC files left behind: {:?}",
        env.ipc_files()
    );
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    pub fn cancelled_batch_build_removes_the_injected_script() {
        let env = TestEnv::new("batch-cancel");
        env.install_fake_editor();

        let ucom = RunningUcom::spawn(
            env.ucom(&["build", "webgl", &env.project_arg(), "-q"])
                .env(FAKE_MODE, "batch-wait"),
        );
        wait_until("the fake Unity to start", || {
            env.root.join("fake.pid").exists()
        });
        assert!(!env.injected_scripts().is_empty(), "no script was injected");

        ucom.ctrl_c();
        let output = ucom.wait();

        assert_eq!(
            output.status.signal(),
            Some(libc::SIGINT),
            "{}",
            describe(&output)
        );
        assert_contains(&stderr(&output), "Cancelled");
        assert!(
            env.injected_scripts().is_empty(),
            "injected script left behind"
        );
        assert!(!env.fake_is_running(), "the fake Unity is still running");
    }

    pub fn second_ctrl_c_kills_a_unity_that_ignores_it() {
        let env = TestEnv::new("batch-force");
        env.install_fake_editor();

        let ucom = RunningUcom::spawn(
            env.ucom(&["build", "webgl", &env.project_arg(), "-q"])
                .env(FAKE_MODE, "batch-ignore-ctrl-c"),
        );
        wait_until("the fake Unity to start", || {
            env.root.join("fake.pid").exists()
        });

        let start = Instant::now();
        ucom.ctrl_c();
        thread::sleep(Duration::from_millis(500));
        // The fake ignored the first Ctrl+C, so ucom is still waiting for it.
        assert!(
            env.fake_is_running(),
            "the fake Unity did not ignore Ctrl+C"
        );
        ucom.ctrl_c();
        let output = ucom.wait();

        // Without the second Ctrl+C, ucom would wait 10 s before killing the fake.
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "took {:?}",
            start.elapsed()
        );
        assert_eq!(
            output.status.signal(),
            Some(libc::SIGINT),
            "{}",
            describe(&output)
        );
        assert!(
            env.injected_scripts().is_empty(),
            "injected script left behind"
        );
        assert!(!env.fake_is_running(), "the fake Unity is still running");
    }

    pub fn cancelled_editor_build_removes_the_command_file() {
        let env = TestEnv::new("editor-cancel");
        env.add_builder();
        // An editor that never answers, as if it were busy building.
        let mut editor = env.start_fake_editor("editor-silent");

        // ucom gets its own process group, so the Ctrl+C does not reach the editor.
        let ucom = RunningUcom::spawn(&mut env.ucom(&["build", "webgl", &env.project_arg()]));
        wait_until("the command file", || !env.ipc_files().is_empty());

        ucom.ctrl_c();
        let output = ucom.wait();

        assert_eq!(
            output.status.signal(),
            Some(libc::SIGINT),
            "{}",
            describe(&output)
        );
        assert_contains(&stderr(&output), "Stopped waiting for the Unity editor");
        assert!(
            env.ipc_files().is_empty(),
            "IPC files left behind: {:?}",
            env.ipc_files()
        );
        assert!(editor.is_running(), "the editor was stopped");
    }
}

//
// Test environment
//

/// A temporary folder with a minimal Unity project, an isolated cache, and room for a fake
/// editor installation. Removed when dropped.
struct TestEnv {
    root: PathBuf,
}

impl TestEnv {
    fn new(label: &str) -> Self {
        let root = env::temp_dir().join(format!("ucom-fake-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let env = Self { root };

        let version_file = env.project().join("ProjectSettings/ProjectVersion.txt");
        fs::create_dir_all(version_file.parent().unwrap()).unwrap();
        fs::write(&version_file, format!("m_EditorVersion: {UNITY_VERSION}\n")).unwrap();
        fs::create_dir_all(env.project().join("Assets")).unwrap();
        env.seed_release_cache();
        env
    }

    fn project(&self) -> PathBuf {
        self.root.join("Project")
    }

    fn project_arg(&self) -> String {
        self.project().to_string_lossy().into_owned()
    }

    fn path_arg(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    /// A `ucom` command with this environment's cache and editor directory.
    fn ucom(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ucom"));
        cmd.args(args)
            .env("UCOM_EDITOR_DIR", self.root.join("Editors"))
            .env("HOME", self.root.join("home"))
            .env("XDG_CACHE_HOME", self.root.join("home/.cache"))
            .env("UCOM_FAKE_DIR", &self.root)
            .env_remove(FAKE_MODE)
            .env("NO_COLOR", "1");
        cmd
    }

    /// Writes a fresh, empty release data cache, so that ucom does not download anything.
    /// On Windows the cache directory cannot be redirected, so ucom uses the real one there.
    fn seed_release_cache(&self) {
        let cache = if cfg!(target_os = "macos") {
            self.root.join("home/Library/Caches/ucom")
        } else {
            self.root.join("home/.cache/ucom")
        };
        fs::create_dir_all(&cache).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        fs::write(
            cache.join("releases_dataset.json"),
            format!(r#"{{"lastUpdate":"{now}","lastFullUpdate":"{now}","suggestedVersion":null,"releases":[]}}"#),
        )
        .unwrap();
    }

    /// Installs this program as the Unity editor in the editor directory, for batch builds.
    fn install_fake_editor(&self) {
        let exe = self
            .root
            .join("Editors")
            .join(UNITY_VERSION)
            .join(EDITOR_EXE_SUB_PATH);
        copy_self_to(&exe);
    }

    /// Installs the builder script, which editor builds require.
    fn add_builder(&self) {
        let output = self
            .ucom(&["add", "builder", &self.project_arg()])
            .output()
            .unwrap();
        assert_success(&output);
    }

    /// Starts this program as an open editor with the project, in the given fake mode.
    fn start_fake_editor(&self, mode: &str) -> FakeEditor {
        let exe = self.root.join("OpenEditor").join(EDITOR_EXE_NAME);
        copy_self_to(&exe);
        let child = Command::new(&exe)
            .args(["-projectPath", &self.project_arg()])
            .env(FAKE_MODE, mode)
            .env("UCOM_FAKE_DIR", &self.root)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // Give the process time to appear in the process list.
        thread::sleep(Duration::from_millis(300));
        FakeEditor(child)
    }

    /// The temporary build script folders in the project's `Assets` folder.
    fn injected_scripts(&self) -> Vec<PathBuf> {
        files_in(&self.project().join("Assets"))
            .into_iter()
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("Ucom-")
            })
            .collect()
    }

    /// The files in the project's editor command and result folders.
    fn ipc_files(&self) -> Vec<PathBuf> {
        let temp = self.project().join("Temp");
        let mut files = files_in(&temp.join("ucom-commands"));
        files.extend(files_in(&temp.join("ucom-results")));
        files
    }

    /// Whether the batch fake whose PID is in `fake.pid` is still running.
    #[cfg(unix)]
    fn fake_is_running(&self) -> bool {
        let pid: i32 = fs::read_to_string(self.root.join("fake.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // SAFETY: signal 0 only checks that the process exists.
        unsafe { libc::kill(pid, 0) == 0 }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A fake editor started by a test. Killed when dropped.
struct FakeEditor(Child);

impl FakeEditor {
    #[cfg(unix)]
    fn is_running(&mut self) -> bool {
        self.0.try_wait().unwrap().is_none()
    }
}

impl Drop for FakeEditor {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(target_os = "macos")]
const EDITOR_EXE_SUB_PATH: &str = "Unity.app/Contents/MacOS/Unity";
#[cfg(target_os = "windows")]
const EDITOR_EXE_SUB_PATH: &str = r"Editor\Unity.exe";
#[cfg(target_os = "linux")]
const EDITOR_EXE_SUB_PATH: &str = "Editor/Unity";

const EDITOR_EXE_NAME: &str = if cfg!(windows) { "Unity.exe" } else { "Unity" };

fn copy_self_to(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::copy(env::current_exe().unwrap(), path).unwrap();
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    match fs::read_dir(dir) {
        Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    }
}

//
// Assertions and process helpers
//

const TIMEOUT: Duration = Duration::from_secs(30);

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn describe(output: &Output) -> String {
    format!(
        "status: {}\n--- stdout\n{}\n--- stderr\n{}",
        output.status,
        stdout(output),
        stderr(output)
    )
}

fn assert_success(output: &Output) {
    assert!(output.status.success(), "ucom failed\n{}", describe(output));
}

fn assert_contains(text: &str, expected: &str) {
    assert!(text.contains(expected), "expected {expected:?} in:\n{text}");
}

/// A ucom process started by a test, in a process group of its own (as a terminal would run
/// it). Its output is read while it runs, so it never blocks on a full pipe. If the test fails
/// before waiting for it, the whole group is killed, including any fake Unity it started.
#[cfg(unix)]
struct RunningUcom {
    child: Option<Child>,
    stdout: Option<thread::JoinHandle<Vec<u8>>>,
    stderr: Option<thread::JoinHandle<Vec<u8>>>,
}

#[cfg(unix)]
impl RunningUcom {
    fn spawn(cmd: &mut Command) -> Self {
        use std::io::Read;
        use std::os::unix::process::CommandExt;

        let mut child = cmd
            .process_group(0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let read_all = |mut pipe: Box<dyn Read + Send>| {
            thread::spawn(move || {
                let mut bytes = Vec::new();
                let _ = pipe.read_to_end(&mut bytes);
                bytes
            })
        };
        let stdout = child.stdout.take().map(|p| read_all(Box::new(p)));
        let stderr = child.stderr.take().map(|p| read_all(Box::new(p)));
        Self {
            child: Some(child),
            stdout,
            stderr,
        }
    }

    /// Sends Ctrl+C to ucom's process group, as a terminal would.
    fn ctrl_c(&self) {
        signal_group(self.child.as_ref().unwrap(), libc::SIGINT);
    }

    /// Waits for ucom to exit, at most [`TIMEOUT`], and returns its output.
    fn wait(mut self) -> Output {
        let mut child = self.child.take().unwrap();
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if start.elapsed() > TIMEOUT {
                self.child = Some(child);
                panic!("ucom did not exit within {TIMEOUT:?}");
            }
            thread::sleep(Duration::from_millis(50));
        };
        // A process ucom started may still hold the pipes; it is part of the group.
        signal_group(&child, libc::SIGKILL);
        let read = |handle: Option<thread::JoinHandle<Vec<u8>>>| {
            handle.map(|h| h.join().unwrap()).unwrap_or_default()
        };
        Output {
            status,
            stdout: read(self.stdout.take()),
            stderr: read(self.stderr.take()),
        }
    }
}

/// Sends `signal` to the process group that `child` leads.
#[cfg(unix)]
fn signal_group(child: &Child, signal: i32) {
    let group = i32::try_from(child.id()).unwrap();
    // SAFETY: signals a process group this test created; no memory is involved.
    unsafe { libc::kill(-group, signal) };
}

#[cfg(unix)]
impl Drop for RunningUcom {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            signal_group(&child, libc::SIGKILL);
            let _ = child.wait();
        }
    }
}

#[cfg(unix)]
fn wait_until(what: &str, condition: impl Fn() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(start.elapsed() < TIMEOUT, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(50));
    }
}

//
// The fake Unity
//

mod fake {
    use super::*;

    /// Runs as a fake Unity. Modes:
    ///
    /// - `batch`: writes its arguments to the `-logFile` log and exits successfully.
    /// - `batch-wait`: writes the log, then runs until Ctrl+C ends it.
    /// - `batch-ignore-ctrl-c`: like `batch-wait`, but ignores Ctrl+C (Unix only).
    /// - `editor`: an open editor that answers build commands with a successful result.
    /// - `editor-silent`: an open editor that never answers.
    ///
    /// Every mode stops by itself after 60 s, so a failed test leaves nothing running.
    pub fn run(mode: &str) -> ExitCode {
        let args: Vec<String> = env::args().collect();
        let arg_after = |flag: &str| {
            args.iter()
                .position(|a| a.eq_ignore_ascii_case(flag))
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        // Tests wait for this file before sending Ctrl+C, so it is written once the fake is set
        // up, not earlier.
        let announce_started = || {
            if let Ok(dir) = env::var("UCOM_FAKE_DIR") {
                let _ = fs::write(
                    Path::new(&dir).join("fake.pid"),
                    std::process::id().to_string(),
                );
            }
        };

        match mode {
            "batch" | "batch-wait" | "batch-ignore-ctrl-c" => {
                #[cfg(unix)]
                if mode == "batch-ignore-ctrl-c" {
                    // SAFETY: changes this process's own signal disposition only.
                    unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
                }
                if let Some(log) = arg_after("-logFile") {
                    let _ = fs::write(&log, format!("fake unity {}\n", args.join(" ")));
                }
                announce_started();
                if mode != "batch" {
                    thread::sleep(Duration::from_secs(60));
                }
                ExitCode::SUCCESS
            }
            "editor" | "editor-silent" => {
                let project = PathBuf::from(arg_after("-projectPath").expect("no -projectPath"));
                announce_started();
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(60) {
                    if mode == "editor" {
                        answer_commands(&project);
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                ExitCode::SUCCESS
            }
            _ => {
                eprintln!("unknown fake mode {mode}");
                ExitCode::FAILURE
            }
        }
    }

    /// Answers each build command file in the project with a successful result, as
    /// `UnityBuilder.cs` does: the log at the command's `log_path`, then the result file.
    fn answer_commands(project: &Path) {
        let commands = project.join("Temp/ucom-commands");
        let results = project.join("Temp/ucom-results");
        for path in files_in(&commands) {
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(command) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let uuid = command["uuid"].as_str().unwrap();
            let log_path = command["log_path"].as_str().unwrap();
            let output_path = command["output_path"].as_str().unwrap();
            let platform = command["platform"].as_str().unwrap();

            fs::write(log_path, "fake editor build\n[Builder] Build succeeded.\n").unwrap();
            let result = serde_json::json!({
                "uuid": uuid,
                "status": "success",
                "message": "Build succeeded",
                "platform_switched": false,
                "original_platform": platform,
                "build_time_seconds": 0.1,
                "platform_switch_time_seconds": 0.0,
                "output_path": output_path,
                "build_result": "Succeeded",
                "platform": platform,
                "total_errors": 0,
                "total_warnings": 0,
                "log_written": true,
            });
            fs::create_dir_all(&results).unwrap();
            let tmp = results.join(format!("build-{uuid}.tmp"));
            fs::write(&tmp, result.to_string()).unwrap();
            fs::rename(&tmp, results.join(format!("build-{uuid}.json"))).unwrap();
            let _ = fs::remove_file(&path);
        }
    }
}

//
// A minimal test runner, since this program also has to run as the fake.
//

mod harness {
    use super::*;

    /// libtest options that take a separate value, which must not be read as a filter.
    const OPTIONS_WITH_VALUE: &[&str] = &[
        "--skip",
        "--test-threads",
        "--format",
        "--color",
        "--logfile",
        "--shuffle-seed",
        "-Z",
    ];

    /// Runs the tests selected by the arguments, like the standard harness: name filters
    /// (substring, or the whole name with `--exact`) and `--skip <pattern>`. Other libtest
    /// options are accepted and ignored.
    pub fn run(tests: &[(&str, fn())]) -> ExitCode {
        let args: Vec<String> = env::args().skip(1).collect();
        if args.iter().any(|a| a == "--list") {
            return ExitCode::SUCCESS;
        }
        let exact = args.iter().any(|a| a == "--exact");
        let mut filters = Vec::new();
        let mut skips = Vec::new();
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            if arg == "--skip" {
                skips.extend(rest.next());
            } else if OPTIONS_WITH_VALUE.contains(&arg.as_str()) {
                rest.next();
            } else if !arg.starts_with('-') {
                filters.push(arg);
            }
        }
        let matches = |name: &str, pattern: &str| {
            if exact {
                name == pattern
            } else {
                name.contains(pattern)
            }
        };
        let selected: Vec<_> = tests
            .iter()
            .filter(|(name, _)| filters.is_empty() || filters.iter().any(|f| matches(name, f)))
            .filter(|(name, _)| !skips.iter().any(|s| matches(name, s)))
            .collect();

        println!("\nrunning {} tests", selected.len());
        let mut failed = Vec::new();
        for (name, test) in &selected {
            match panic::catch_unwind(test) {
                Ok(()) => println!("test {name} ... ok"),
                Err(_) => {
                    println!("test {name} ... FAILED");
                    failed.push(*name);
                }
            }
        }

        let result = if failed.is_empty() { "ok" } else { "FAILED" };
        println!(
            "\ntest result: {result}. {} passed; {} failed\n",
            selected.len() - failed.len(),
            failed.len()
        );
        if failed.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }
}
