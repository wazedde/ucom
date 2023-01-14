#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::str::{FromStr, Split};

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

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub enum BuildType {
    Alpha,
    Beta,
    ReleaseCandidate,
    Final,
}

pub struct BuildTypeParseError;

impl BuildType {
    pub fn as_str(&self) -> &str {
        match self {
            BuildType::Alpha => "a",
            BuildType::Beta => "b",
            BuildType::ReleaseCandidate => "rc",
            BuildType::Final => "f",
        }
    }

    pub fn find_in(s: &str) -> Option<BuildType> {
        if s.contains('f') {
            Some(BuildType::Final)
        } else if s.contains('b') {
            Some(BuildType::Beta)
        } else if s.contains('a') {
            Some(BuildType::Alpha)
        } else if s.contains("rc") {
            Some(BuildType::ReleaseCandidate)
        } else {
            None
        }
    }
}

impl Display for BuildType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for BuildType {
    type Err = BuildTypeParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "a" => Ok(BuildType::Alpha),
            "b" => Ok(BuildType::Beta),
            "rc" => Ok(BuildType::ReleaseCandidate),
            "f" => Ok(BuildType::Final),
            _ => Err(BuildTypeParseError),
        }
    }
}

/// The Unity version separated into its components.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct UnityVersion {
    pub year: u16,
    pub point: u8,
    pub patch: u8,
    pub build_type: BuildType,
    pub build: u8,
}

pub struct UnityVersionParseError;

impl FromStr for UnityVersion {
    type Err = UnityVersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let year = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(UnityVersionParseError)?;
        let point = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(UnityVersionParseError)?;

        let build_type = BuildType::find_in(s).ok_or(UnityVersionParseError)?;

        let mut build_parts: Split<&str> = parts
            .next()
            .ok_or(UnityVersionParseError)?
            .split(build_type.as_str());
        let patch = build_parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(UnityVersionParseError)?;
        let build = build_parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or(UnityVersionParseError)?;

        Ok(UnityVersion {
            year,
            point,
            patch,
            build_type,
            build,
        })
    }
}

impl Display for UnityVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}{}{}",
            self.year, self.point, self.patch, self.build_type, self.build
        )
    }
}
