#![allow(dead_code)]

use indexmap::IndexMap;
use select::document::Document;
use select::predicate::{Class, Name};

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
