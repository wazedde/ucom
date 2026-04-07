use yansi::Color::{Blue, BrightBlack, BrightBlue, Green, Red, Yellow};
use yansi::Style;

pub static STYLE_PLAIN: Style = Style::new();
pub static STYLE_SUCCESS: Style = Green.foreground();
pub static STYLE_ERROR: Style = Red.foreground();
pub static STYLE_WARNING: Style = Yellow.foreground();
pub static STYLE_LINK: Style = BrightBlue.underline();
pub static STYLE_UP_TO_DATE: Style = Green.foreground();
pub static STYLE_UPDATE_AVAILABLE: Style = Yellow.foreground();
pub static STYLE_UPDATE_VERSION: Style = Blue.foreground();
pub static STYLE_NO_UPDATE_INFO: Style = BrightBlack.foreground();
