//! How the commands show known issues of a Unity release.

use crate::commands::output::{MARK_BULLET, MARK_ERROR, MARK_WARNING};
use crate::style_definitions::{STYLE_ERROR, STYLE_LINK, STYLE_PLAIN, STYLE_WARNING};
use crate::unity::Version;
use crate::unity::release_api::{UpdatePolicy, fetch_latest_releases};
use crate::unity::release_api_data::{LabelElement, ReleaseIssue};
use crate::utils::report::{HeaderLevel, Report, WrapMode};
use crate::utils::warn_non_fatal;
use std::borrow::Cow;
use yansi::{Paint, Style};

/// Terminal styling for a [`ReleaseIssue`].
pub trait ReleaseIssueDisplay {
    /// The style for the issue: error, warning, or plain without an issue.
    fn style(&self) -> Style;

    /// Like [`style`](Self::style), but `style` when there is no issue.
    fn issue_style_or(&self, style: Style) -> Style;

    /// A styled mark for the issue; a bullet without an issue.
    fn marker(&self) -> String;

    /// Like [`marker`](Self::marker), but `fallback` when there is no issue.
    fn issue_marker_or(&self, fallback: &'static str) -> Cow<'static, str>;

    /// Like [`issue_tag`](Self::issue_tag), but `fallback` when there is no issue.
    fn issue_tag_or(&self, fallback: &'static str) -> Cow<'static, str>;

    /// The issue's label as a styled tag after `prefix`; empty without an issue.
    fn issue_tag_with_prefix(&self, prefix: &str) -> String;

    /// The issue's label as a styled tag; empty without an issue.
    fn issue_tag(&self) -> String {
        self.issue_tag_with_prefix("")
    }
}

impl ReleaseIssueDisplay for ReleaseIssue {
    fn style(&self) -> Style {
        self.issue_style_or(STYLE_PLAIN)
    }

    fn issue_style_or(&self, style: Style) -> Style {
        match self {
            ReleaseIssue::NoIssue => style,
            ReleaseIssue::Error(_) => STYLE_ERROR,
            ReleaseIssue::Warning(_) => STYLE_WARNING,
        }
    }

    fn marker(&self) -> String {
        let mark = match self {
            ReleaseIssue::NoIssue => MARK_BULLET,
            ReleaseIssue::Error(_) => MARK_ERROR,
            ReleaseIssue::Warning(_) => MARK_WARNING,
        };
        mark.paint(self.style()).to_string()
    }

    fn issue_marker_or(&self, fallback: &'static str) -> Cow<'static, str> {
        if self.has_issue() {
            Cow::Owned(self.marker())
        } else {
            Cow::Borrowed(fallback)
        }
    }

    fn issue_tag_or(&self, fallback: &'static str) -> Cow<'static, str> {
        if self.has_issue() {
            Cow::Owned(self.issue_tag())
        } else {
            Cow::Borrowed(fallback)
        }
    }

    fn issue_tag_with_prefix(&self, prefix: &str) -> String {
        self.label().map_or_default(|label| {
            format!("{prefix}[{}]", format_label_with_url(label, self.style()))
        })
    }
}

pub fn report_error_description(report: &Report, issue: &ReleaseIssue) {
    let Some(label) = issue.label() else {
        return;
    };

    report.blank_line();
    report.header(
        format_args!("{}", label.label_text.to_uppercase()).paint(issue.style()),
        HeaderLevel::H2,
    );

    let description = report.render_links(&label.description, STYLE_PLAIN, STYLE_LINK);
    let description = report.wrap_text(&description, WrapMode::TerminalWidth);
    report.paragraph(&description);
}

/// Checks if the given version has any issues and reports them.
pub fn report_version_issues(unity_version: Version) {
    let releases = match fetch_latest_releases(UpdatePolicy::Incremental) {
        Ok(releases) => releases,
        Err(e) => {
            warn_non_fatal!("Failed to fetch release information: {e}");
            return;
        }
    };

    let release_data = match releases.get_by_version(unity_version) {
        Ok(release_data) => release_data,
        Err(e) => {
            warn_non_fatal!("Failed to get release data for version {unity_version}: {e}");
            return;
        }
    };

    let issue = release_data.issue();
    if issue.has_issue() {
        let report = Report::Terminal;
        report_error_description(&report, &issue);
        report.blank_line();
    }
}

fn format_label_with_url(le: &LabelElement, style: Style) -> String {
    extract_first_url(&le.description).map_or_else(
        || le.label_text.to_uppercase().paint(style).to_string(),
        |url| {
            le.label_text
                .to_uppercase()
                .paint(style)
                .link(url)
                .to_string()
        },
    )
}

fn extract_first_url(text: &str) -> Option<&str> {
    let start_index = text.find("http://").or_else(|| text.find("https://"))?;
    let remaining_text = &text[start_index..];
    let end_index = remaining_text
        .find(|c: char| c.is_whitespace() || c == '"' || c == '<' || c == '>')
        .unwrap_or(remaining_text.len());

    Some(&remaining_text[..end_index])
}
