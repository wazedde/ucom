//! Terminal output shared by the commands: indentation, marks, tags and formatting helpers.

use crate::style_definitions::{STYLE_OUTDATED, STYLE_UP_TO_DATE, STYLE_UPDATE_VERSION};
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
