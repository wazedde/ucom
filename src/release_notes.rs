#![allow(dead_code)]

use indexmap::IndexMap;
use select::document::Document;
use select::predicate::{Class, Name};

pub(crate) fn release_notes_url<P: AsRef<str>>(version: P) -> String {
    // remove the patch version.
    let version = version.as_ref().split('f').next().unwrap();
    format!("https://unity.com/releases/editor/whats-new/{}", version)
}

pub(crate) fn collect_release_notes(html: &str) -> IndexMap<String, Vec<String>> {
    let document = Document::from(html);
    let mut release_notes = IndexMap::<String, Vec<String>>::new();

    if let Some(node) = document.find(Class("release-notes")).next() {
        let mut last_header = String::new();
        node.children().for_each(|n| match n.name() {
            Some("h3") => last_header = n.text(),
            Some("h4") => last_header = n.text(),
            Some("ul") => {
                if !release_notes.contains_key(&last_header) {
                    release_notes.insert(last_header.clone(), Vec::new());
                }

                let list = release_notes.get_mut(&last_header).unwrap();
                n.find(Name("li")).for_each(|li| {
                    if let Some(s) = li.text().lines().next() {
                        list.push(s.to_string());
                    }
                });
            }
            _ => {}
        })
    }

    release_notes
}
