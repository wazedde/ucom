pub use crate::commands::build_project::build_project;
pub use crate::commands::check_updates::check_updates;
pub use crate::commands::list_versions::list_versions;
pub use crate::commands::new_project::new_project;
pub use crate::commands::open_project::open_project;
pub use crate::commands::project_info::project_info;
pub use crate::commands::run_unity::run_unity;
use colored::{ColoredString, Colorize};

pub mod build_project;
pub mod check_updates;
pub mod list_versions;
pub mod new_project;
pub mod open_project;
pub mod project_info;
pub mod run_unity;
pub mod terminal_spinner;

pub trait ColoredStringIf {
    /// Returns bold string if `is_bold` is true.
    fn bold_if(&self, is_bold: bool) -> ColoredString;
}

impl ColoredStringIf for str {
    fn bold_if(&self, is_bold: bool) -> ColoredString {
        if is_bold {
            self.bold()
        } else {
            self.into()
        }
    }
}
