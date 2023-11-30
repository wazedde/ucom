use std::path::Path;

use colored::Colorize;
use itertools::Itertools;

use crate::cli::PackagesInfoLevel;
use crate::unity::project::*;
use crate::unity::{release_notes_url, validate_existing_dir, ProjectPath};

/// Shows project information.
pub fn project_info(
    path: &Path,
    packages_level: PackagesInfoLevel,
    recursive: bool,
) -> anyhow::Result<()> {
    if !recursive {
        return print_project_info(&ProjectPath::from(path)?, packages_level);
    }

    let absolute_path = validate_existing_dir(&path)?;
    println!(
        "Searching for Unity projects in: {}",
        absolute_path.display(),
    );

    let mut it = recursive_dir_iter(absolute_path);
    while let Some(Ok(entry)) = it.next() {
        if let Ok(path) = ProjectPath::from(entry.path()) {
            println!();
            if let Err(err) = print_project_info(&path, packages_level) {
                println!("    {}", err.to_string().red());
            }
            it.skip_current_dir();
        }
    }
    Ok(())
}

fn print_project_info(
    project: &ProjectPath,
    packages_level: PackagesInfoLevel,
) -> anyhow::Result<()> {
    let unity_version = project.unity_version()?;

    println!(
        "{}",
        format!("Project info for: {}", project.as_path().display()).bold()
    );

    match Settings::from_project(project) {
        Ok(settings) => {
            let ps = settings.player_settings;
            println!("    Product Name:  {}", ps.product_name.bold());
            println!("    Company Name:  {}", ps.company_name.bold());
            println!("    Version:       {}", ps.bundle_version.bold());
        }

        Err(e) => {
            println!(
                "    {}: {}",
                "No project settings found".yellow(),
                e.to_string().yellow()
            );
        }
    }

    print!(
        "    Unity Version: {} - {}",
        unity_version.to_string().bold(),
        release_notes_url(unity_version).bright_blue()
    );

    if unity_version.is_editor_installed()? {
        println!();
    } else {
        println!(" {}", "*not installed".red().bold());
    }

    if packages_level != PackagesInfoLevel::None {
        print_project_packages(project, packages_level)?;
    };

    Ok(())
}

/// Show packages used by the project.
fn print_project_packages(
    project: &ProjectPath,
    package_level: PackagesInfoLevel,
) -> anyhow::Result<()> {
    let availability = Packages::from_project(project)?;

    match availability {
        PackagesAvailability::NoManifest => {
            println!(
                "    {}",
                "No `manifest.json` file found, no packages info available.".yellow()
            );
            Ok(())
        }
        PackagesAvailability::LockFileDisabled => {
            println!(
                "    {}",
                "Packages lock file is disabled in `manifest.json`, no packages info available."
                    .yellow()
            );
            Ok(())
        }
        PackagesAvailability::NoLockFile => {
            println!(
                "    {}",
                "No `packages-lock.json` file found, no packages info available.".yellow()
            );
            Ok(())
        }
        PackagesAvailability::Packages(packages) => {
            let mut packages = packages
                .dependencies
                .iter()
                .filter(|(name, package)| package_level.evaluate(name, package))
                .collect_vec();

            if packages.is_empty() {
                return Ok(());
            }

            packages.sort_by(|(_, pi1), (_, pi2)| pi1.source.cmp(&pi2.source));

            println!();
            println!(
                "    {} {} {}",
                "Packages:".bold(),
                package_level.to_string().bold(),
                "(L=local, E=embedded, G=git, T=tarball, R=registry, B=builtin)".bold()
            );

            for (name, package) in packages {
                println!(
                    "    {} {} ({})",
                    package.source.as_ref().map_or("?", |s| s.to_short_str()),
                    name,
                    package.version,
                );
            }
            Ok(())
        }
    }
}

impl PackagesInfoLevel {
    /// Evaluates if the `PackageInfo` is allowed by the info level.
    fn evaluate(self, name: &str, package: &PackageInfo) -> bool {
        match self {
            Self::None => false,

            Self::ExcludingUnity => {
                package.depth == 0
                    && package.source.map_or(false, |ps| {
                        ps < PackageSource::Registry
                            || (ps == PackageSource::Registry
                                && package
                                    .url
                                    .as_ref()
                                    .map_or(false, |u| u != "https://packages.unity.com"))
                    })
            }

            Self::IncludingUnity => {
                package.depth == 0
                    && package.source.map_or(false, |ps| {
                        ps < PackageSource::Builtin || name.starts_with("com.unity.feature.")
                    })
            }

            Self::All => true,
        }
    }
}
