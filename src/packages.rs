use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub(crate) struct Manifest {
    pub(crate) dependencies: BTreeMap<String, String>,
}

#[allow(dead_code)]
impl Manifest {
    pub(crate) fn from_project(project_dir: &Path) -> Result<Manifest> {
        let file_name: &str = "Packages/manifest.json";
        let path = project_dir.join(file_name);

        let file =
            File::open(&path).context(format!("Cannot read '{}'", path.to_string_lossy()))?;

        let manifest: Manifest = serde_json::from_reader(BufReader::new(file))
            .context(format!("Cannot parse '{}'", path.to_string_lossy()))?;

        Ok(manifest)
    }
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub(crate) struct PackageInfo {
    pub(crate) version: String,
    pub(crate) depth: u32,
    pub(crate) source: String,
    pub(crate) dependencies: BTreeMap<String, String>,
    pub(crate) url: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub(crate) struct Packages {
    pub(crate) dependencies: BTreeMap<String, PackageInfo>,
}

impl Packages {
    pub(crate) fn from_project(project_dir: &Path) -> Result<Packages> {
        let file_name: &str = "Packages/packages-lock.json";
        let path = project_dir.join(file_name);

        let file =
            File::open(&path).context(format!("Cannot read '{}'", path.to_string_lossy()))?;

        let manifest: Packages = serde_json::from_reader(BufReader::new(file))
            .context(format!("Cannot parse '{}'", path.to_string_lossy()))?;

        Ok(manifest)
    }
}
