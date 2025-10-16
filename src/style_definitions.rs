use yansi::Color::{Blue, BrightBlack, BrightBlue, Green, Red, White, Yellow};
use yansi::Style;

pub static PLAIN: Style = Style::new();
pub static NORMAL: Style = White.foreground();
pub static OK: Style = Green.foreground();
pub static ERROR: Style = Red.foreground();
pub static WARNING: Style = Yellow.foreground();
pub static LINK: Style = BrightBlue.underline();
pub static UP_TO_DATE: Style = Green.foreground();
pub static HAS_UPDATE: Style = Yellow.foreground();
pub static IS_UPDATE: Style = Blue.foreground();
pub static NO_UPDATE_INFO: Style = BrightBlack.foreground();
