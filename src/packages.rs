use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub(crate) struct Manifest {
    pub(crate) dependencies: BTreeMap<String, String>,
}

impl Manifest {
    pub(crate) fn from_project(project_dir: &Path) -> Result<Manifest> {
        const MANIFEST: &str = "Packages/manifest.json";
        let path = project_dir.join(MANIFEST);

        let file =
            File::open(&path).context(format!("Cannot read '{}'", path.to_string_lossy()))?;

        let manifest: Manifest = serde_json::from_reader(BufReader::new(file))
            .context(format!("Cannot parse '{}'", path.to_string_lossy()))?;

        Ok(manifest)
    }
}
