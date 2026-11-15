//! Terminal output shared by the commands: indentation, marks, tags and formatting helpers.

use crate::style_definitions::{STYLE_OUTDATED, STYLE_UP_TO_DATE, STYLE_UPDATE_VERSION};
use crate::unity::Version;
use std::fmt::{Display, Formatter};
use std::sync::LazyLock;
use yansi::Paint;

pub const INDENT: &str = "  ";
pub const MARK_NO_INFO: &str = "?";
pub const MARK_BULLET: &str = "-";
pub const MARK_AVAILABLE: &str = "✓";
pub const MARK_UNAVAILABLE: &str = "✗";
pub const MARK_ERROR: &str = "‼";
pub const MARK_WARNING: &str = "!";

pub static TAG_OUTDATED: LazyLock<String> =
    LazyLock::new(|| format!("[{}]", "OUTDATED".paint(STYLE_OUTDATED)));

pub static TAG_NEW: LazyLock<String> =
    LazyLock::new(|| format!("[{}]", "NEW".paint(STYLE_UPDATE_VERSION)));

pub static TAG_UP_TO_DATE: LazyLock<String> =
    LazyLock::new(|| format!("[{}]", "OK".paint(STYLE_UP_TO_DATE)));

/// Formats a left-aligned version.
pub struct AlignedVersion(pub Version, pub usize);

impl Display for AlignedVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Needs to be a string otherwise padding won't work
        let v = self.0.to_interned_str();
        let w = self.1;
        write!(f, "{v:<w$}")
    }
}
