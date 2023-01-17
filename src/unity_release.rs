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

pub async fn fetch_unity_releases() -> Result<Vec<ReleaseInfo>> {
    let url = "https://unity.com/releases/editor/archive";
    let body = reqwest::get(url).await?.text().await?;
    let releases = find_releases(&body, ReleaseFilter::All);
    Ok(releases)
}

fn find_releases(html: &str, filter: ReleaseFilter) -> Vec<ReleaseInfo> {
    let year_class: Cow<str> = match filter {
        ReleaseFilter::All => "release-tab-content".into(),
        ReleaseFilter::Year { year } => year.to_string().into(),
        ReleaseFilter::Point { year, .. } => year.to_string().into(),
    };

    let mut versions: Vec<_> = Document::from(html)
        .find(Class(year_class.as_ref()))
        .flat_map(|n| n.find(Class("download-release-wrapper")))
        .flat_map(|n| {
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
        .flat_map(|(date_header, url)| {
            version_from_url(url)
                .filter(|v| match filter {
                    ReleaseFilter::All => true,
                    ReleaseFilter::Year { year } => v.year == year,
                    ReleaseFilter::Point { year, point } => v.year == year && v.point == point,
                })
                .map(|version| ReleaseInfo {
                    version,
                    date_header,
                    installation_url: url.to_owned(),
                })
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

pub fn release_notes_url<P: AsRef<str>>(version: P) -> String {
    // remove the patch version.
    let version = version.as_ref().split('f').next().unwrap();
    format!("https://unity.com/releases/editor/whats-new/{}", version)
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
        })
    }

    release_notes
}
