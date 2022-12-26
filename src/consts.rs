pub(crate) const ENV_EDITOR_DIR: &str = "UCOM_EDITOR_DIR";

/// Sub path to the executable on macOS.
#[cfg(target_os = "macos")]
pub(crate) const UNITY_EDITOR_EXE: &str = "Unity.app/Contents/MacOS/Unity";

/// Sub path to the executable on Windows.
#[cfg(target_os = "windows")]
pub(crate) const UNITY_EDITOR_EXE: &str = r"Editor\Unity.exe";

/// Other target platforms are not supported.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) const UNITY_EDITOR_EXE: &str = compile_error!("Unsupported platform");

/// Parent directory of editor installations on macOS.
#[cfg(target_os = "macos")]
pub(crate) const UNITY_EDITOR_DIR: &str = "/Applications/Unity/Hub/Editor/";

/// Parent directory of editor installations on Windows.
#[cfg(target_os = "windows")]
pub(crate) const UNITY_EDITOR_DIR: &str = r"C:\Program Files\Unity\Hub\Editor";

/// Other target platforms are not supported.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) const UNITY_EDITOR_DIR: &str = compile_error!("Unsupported platform");

pub(crate) const BUILD_SCRIPT: &str = include_str!("include/UcomBuilder.cs");

pub(crate) const BUILD_SCRIPT_NAME: &str = "UcomBuilder.cs";
pub(crate) const PERSISTENT_BUILD_SCRIPT_PATH: &str = "Assets/Plugins/ucom/Editor/UcomBuilder.cs";
pub(crate) const PERSISTENT_BUILD_SCRIPT_ROOT: &str = "Assets/Plugins/ucom";
pub(crate) const AUTO_BUILD_SCRIPT_ROOT: &str = "Assets/ucom";
