use crate::unity::release_api::{get_latest_releases, Mode};
use crate::unity::release_api_data::ReleaseData;
use yansi::Paint;

pub(crate) fn install_partial_version(partial_version: &str, mode: Mode) -> anyhow::Result<()> {
    let releases = get_latest_releases(mode)?;
    let release = releases
        .iter()
        .filter(|rd| rd.version.to_string().starts_with(partial_version))
        .max_by(|a, b| a.version.cmp(&b.version))
        .ok_or(anyhow::anyhow!(
            "No version found that matches `{}`",
            partial_version
        ))?;

    install_version(release)
}

pub(crate) fn install_version(release: &ReleaseData) -> anyhow::Result<()> {
    if release.version.is_editor_installed()? {
        anyhow::bail!("Version {} is already installed", release.version);
    }

    println!(
        "Opening Unity Hub with deep link {} to install version {}",
        release.unity_hub_deep_link.bright_blue(),
        release.version.bold()
    );

    let deep_link = release.unity_hub_deep_link.as_str();

    if deep_link.is_empty() {
        anyhow::bail!(
            "No Unity Hub deep link available for version {}",
            release.version
        );
    }

    let status = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/C", "start", deep_link])
            .status()?
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .args([deep_link])
            .status()?
    } else {
        anyhow::bail!("Unsupported OS for Unity Hub deep linking");
    };

    if !status.success() {
        anyhow::bail!("Failed to open Unity Hub deep link");
    }

    Ok(())
}
