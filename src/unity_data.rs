#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub dependencies: BTreeMap<String, String>,
}

impl Manifest {
    pub fn from_project(project_dir: &Path) -> Result<Manifest> {
        let file = File::open(project_dir.join("Packages/manifest.json"))?;
        serde_json::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}

#[derive(Deserialize, Debug)]
pub struct PackageInfo {
    pub version: String,
    pub depth: u32,
    pub source: String,
    pub dependencies: BTreeMap<String, String>,
    pub url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Packages {
    pub dependencies: BTreeMap<String, PackageInfo>,
}

impl Packages {
    pub fn from_project(project_dir: &Path) -> Result<Packages> {
        let file = File::open(project_dir.join("Packages/packages-lock.json"))?;
        serde_json::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}

#[derive(Deserialize, Debug)]
pub struct ProjectSettings {
    #[serde(rename = "PlayerSettings")]
    pub player_settings: PlayerSettings,
}

#[derive(Deserialize, Debug)]
pub struct PlayerSettings {
    #[serde(rename = "productName")]
    pub product_name: String,

    #[serde(rename = "companyName")]
    pub company_name: String,

    #[serde(rename = "bundleVersion")]
    pub bundle_version: String,

    #[serde(rename = "buildNumber")]
    pub build_number: Option<HashMap<String, String>>,

    #[serde(rename = "AndroidBundleVersionCode")]
    pub android_bundle_version_code: Option<String>,
}

impl ProjectSettings {
    pub fn from_project(project_dir: &Path) -> Result<ProjectSettings> {
        let file = File::open(project_dir.join("ProjectSettings/ProjectSettings.asset"))?;
        serde_yaml::from_reader(BufReader::new(file)).map_err(Into::into)
    }
}
