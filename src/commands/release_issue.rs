//! How the commands show known issues of a Unity release.

use crate::commands::output::{Mark, Tag};
use crate::style_definitions::{Styled, Tone};
use crate::unity::Version;
use crate::unity::release_api::{UpdatePolicy, fetch_latest_releases};
use crate::unity::release_api_data::{LabelElement, ReleaseIssue};
use crate::utils::report::{HeaderLevel, Report, WrapMode};
use crate::utils::warn_non_fatal;
use std::fmt::{self, Display, Formatter};
use std::io::IsTerminal;

/// Terminal styling for a release's issue.
impl ReleaseIssue {
    /// The tone for the issue: error, warning, or plain without an issue.
    pub fn tone(&self) -> Tone {
        self.tone_or(Tone::Plain)
    }

    /// Like [`tone`](Self::tone), but `tone` when there is no issue.
    pub fn tone_or(&self, tone: Tone) -> Tone {
        match self {
            ReleaseIssue::NoIssue => tone,
            ReleaseIssue::Error(_) => Tone::Error,
            ReleaseIssue::Warning(_) => Tone::Warning,
        }
    }

    /// The mark for the issue; a bullet without an issue.
    pub fn mark(&self) -> Mark {
        self.mark_or(Mark::Bullet)
    }

    /// Like [`mark`](Self::mark), but `fallback` when there is no issue.
    pub fn mark_or(&self, fallback: Mark) -> Mark {
        match self {
            ReleaseIssue::NoIssue => fallback,
            ReleaseIssue::Error(_) => Mark::Error,
            ReleaseIssue::Warning(_) => Mark::Warning,
        }
    }

    /// The issue's label as a tag after a space, such as ` [SECURITY ALERT]`; nothing without
    /// an issue.
    pub fn issue_tag(&self) -> IssueTag<'_> {
        IssueTag {
            label: self.label(),
            tone: self.tone(),
            fallback: None,
        }
    }

    /// Like [`issue_tag`](Self::issue_tag), but `fallback` when there is no issue.
    pub fn issue_tag_or(&self, fallback: Tag) -> IssueTag<'_> {
        IssueTag {
            fallback: Some(fallback),
            ..self.issue_tag()
        }
    }
}

/// A release's issue label as a tag after a space, such as ` [SECURITY ALERT]`, linked to the
/// first URL in its description. Shows the fallback tag, or nothing, for a release without an
/// issue.
pub struct IssueTag<'a> {
    label: Option<&'a LabelElement>,
    tone: Tone,
    fallback: Option<Tag>,
}

impl Display for IssueTag<'_> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match (self.label, self.fallback) {
            (Some(label), _) => {
                let text = label.label_text.to_uppercase();
                let text = text.tone(self.tone);
                // A link only on a terminal: forced colour must not put link codes in logs.
                match extract_first_url(&label.description) {
                    Some(url) if std::io::stdout().is_terminal() => {
                        write!(f, " [{}]", text.link(url))
                    }
                    _ => write!(f, " [{text}]"),
                }
            }
            (None, Some(tag)) => write!(f, " {tag}"),
            (None, None) => Ok(()),
        }
    }
}

pub fn report_error_description(report: &Report, issue: &ReleaseIssue) {
    let Some(label) = issue.label() else {
        return;
    };

    report.blank_line();
    report.header(
        format_args!("{}", label.label_text.to_uppercase()).tone(issue.tone()),
        HeaderLevel::H2,
    );

    let description = report.render_links(&label.description, Tone::Plain, Tone::Link);
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

fn extract_first_url(text: &str) -> Option<&str> {
    let start_index = text.find("http://").or_else(|| text.find("https://"))?;
    let remaining_text = &text[start_index..];
    let end_index = remaining_text
        .find(|c: char| c.is_whitespace() || c == '"' || c == '<' || c == '>')
        .unwrap_or(remaining_text.len());

    Some(&remaining_text[..end_index])
}
