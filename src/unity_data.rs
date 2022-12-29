#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub(crate) struct Manifest {
    pub(crate) dependencies: BTreeMap<String, String>,
}

impl Manifest {
    pub(crate) fn from_project(project_dir: &Path) -> Result<Manifest> {
        let file = File::open(project_dir.join("Packages/manifest.json"))?;
        serde_json::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct PackageInfo {
    pub(crate) version: String,
    pub(crate) depth: u32,
    pub(crate) source: String,
    pub(crate) dependencies: BTreeMap<String, String>,
    pub(crate) url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Packages {
    pub(crate) dependencies: BTreeMap<String, PackageInfo>,
}

impl Packages {
    pub(crate) fn from_project(project_dir: &Path) -> Result<Packages> {
        let file = File::open(project_dir.join("Packages/packages-lock.json"))?;
        serde_json::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct ProjectSettings {
    #[serde(rename = "PlayerSettings")]
    pub(crate) player_settings: PlayerSettings,
}

#[derive(Deserialize, Debug)]
pub(crate) struct PlayerSettings {
    #[serde(rename = "productName")]
    pub(crate) product_name: String,
    #[serde(rename = "companyName")]
    pub(crate) company_name: String,
    #[serde(rename = "bundleVersion")]
    pub(crate) bundle_version: String,
    #[serde(rename = "buildNumber")]
    pub(crate) build_number: Option<HashMap<String, String>>,
    #[serde(rename = "AndroidBundleVersionCode")]
    pub(crate) android_bundle_version_code: Option<String>,
}

impl ProjectSettings {
    pub(crate) fn from_project(project_dir: &Path) -> Result<ProjectSettings> {
        let file = File::open(project_dir.join("ProjectSettings/ProjectSettings.asset"))?;
        serde_yaml::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}
