use std::borrow::Cow;

use anyhow::Result;
use indexmap::IndexMap;
use select::document::Document;
use select::predicate::{Class, Name};

use crate::unity_version::*;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ReleaseInfo {
    pub version: UnityVersion,
    pub date_header: String,
    pub installation_url: String,
}

impl ReleaseInfo {
    fn new(version: UnityVersion, date_header: String, installation_url: String) -> Self {
        Self {
            version,
            date_header,
            installation_url,
        }
    }
}

#[allow(dead_code)]
pub enum ReleaseFilter {
    All,
    Year {
        year: VersionYear,
    },
    Point {
        year: VersionYear,
        point: VersionPoint,
    },
}

impl ReleaseFilter {
    fn eval(&self, v: &UnityVersion) -> bool {
        match self {
            ReleaseFilter::All => true,
            ReleaseFilter::Year { year } => v.year == *year,
            ReleaseFilter::Point { year, point } => v.year == *year && v.point == *point,
        }
    }
}

/// Gets Unity releases from the Unity website.
pub fn request_unity_releases() -> Result<Vec<ReleaseInfo>> {
    let url = "https://unity.com/releases/editor/archive";
    let body = ureq::get(url).call()?.into_string()?;

    let releases = find_releases(&body, &ReleaseFilter::All);
    Ok(releases)
}

/// Gets updates for the given version from the Unity website.
pub fn request_updates_for(version: UnityVersion) -> Result<Vec<ReleaseInfo>> {
    let url = "https://unity.com/releases/editor/archive";
    let body = ureq::get(url).call()?.into_string()?;

    let releases = find_releases(
        &body,
        &ReleaseFilter::Point {
            year: version.year,
            point: version.point,
        },
    )
    .into_iter()
    .filter(|ri| ri.version > version)
    .collect();

    Ok(releases)
}

/// Finds releases in the html that match the filter.
fn find_releases(html: &str, filter: &ReleaseFilter) -> Vec<ReleaseInfo> {
    let year_class: Cow<str> = match filter {
        ReleaseFilter::All => "release-tab-content".into(),
        ReleaseFilter::Year { year } | ReleaseFilter::Point { year, .. } => year.to_string().into(),
    };

    let mut versions: Vec<_> = Document::from(html)
        .find(Class(year_class.as_ref()))
        .flat_map(|n| n.find(Class("download-release-wrapper")))
        .filter_map(|n| {
            n.find(Class("release-title-date"))
                .next()
                // Get the release date.
                .and_then(|n| n.find(Class("release-date")).next())
                .map(|n| n.text())
                .and_then(|date_header| {
                    // Get the Unity Hub installation url.
                    n.find(Class("btn"))
                        .next()
                        .and_then(|n| n.attr("href"))
                        .map(|url| (date_header, url))
                })
        })
        .filter_map(|(date_header, url)| {
            version_from_url(url)
                .filter(|v| filter.eval(v))
                .map(|version| ReleaseInfo::new(version, date_header, url.to_owned()))
        })
        .collect();

    versions.sort();
    versions
}

/// Get the version from the url.
/// The url looks like: unityhub://2021.2.14f1/bcb93e5482d2
fn version_from_url(url: &str) -> Option<UnityVersion> {
    url.split('/')
        .rev()
        .nth(1)
        .and_then(|v| v.parse::<UnityVersion>().ok())
}

pub fn release_notes_url(version: UnityVersion) -> String {
    let version = format!("{}.{}.{}", version.year, version.point, version.patch);
    format!("https://unity.com/releases/editor/whats-new/{version}")
}

pub fn collect_release_notes(html: &str) -> IndexMap<String, Vec<String>> {
    let document = Document::from(html);
    let mut release_notes = IndexMap::<String, Vec<String>>::new();

    if let Some(node) = document.find(Class("release-notes")).next() {
        let mut topic_header = "General".to_string();
        node.children().for_each(|n| match n.name() {
            Some("h3") => topic_header = n.text(),
            Some("h4") => topic_header = n.text(),
            Some("ul") => {
                if !release_notes.contains_key(&topic_header) {
                    release_notes.insert(topic_header.clone(), Vec::new());
                }

                let topic_list = release_notes.get_mut(&topic_header).unwrap();
                n.find(Name("li")).for_each(|li| {
                    if let Some(release_note_line) = li.text().lines().next() {
                        topic_list.push(release_note_line.to_string());
                    }
                });
            }
            _ => {}
        });
    }

    release_notes
}
