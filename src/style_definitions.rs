use yansi::Color::{Blue, BrightBlack, BrightBlue, Cyan, Green, Red, Yellow};
use yansi::Style;

/// Default style for plain text.
pub static STYLE_PLAIN: Style = Style::new();
/// Style for success messages and indicators.
pub static STYLE_SUCCESS: Style = Green.foreground();
/// Style for error messages, critical failures, and "not installed" status.
pub static STYLE_ERROR: Style = Red.foreground();
/// Style for warnings and non-critical issues.
pub static STYLE_WARNING: Style = Yellow.foreground();
/// Style for clickable links and URLs.
pub static STYLE_LINK: Style = BrightBlue.underline();
/// Style for "up-to-date" status and latest versions.
pub static STYLE_UP_TO_DATE: Style = Green.foreground();
/// Style for "update available" status and indicators.
pub static STYLE_OUTDATED: Style = Cyan.foreground();
/// Style for displaying update versions.
pub static STYLE_UPDATE_VERSION: Style = Blue.foreground();
/// Style for text indicating no update information is available.
pub static STYLE_NO_UPDATE_INFO: Style = BrightBlack.foreground();
