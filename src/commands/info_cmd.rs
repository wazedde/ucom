use std::path::Path;

use itertools::Itertools;
use yansi::Paint;

use crate::cli::PackagesInfoLevel;
use crate::commands::INDENT;
use crate::unity::project::*;
use crate::unity::{release_notes_url, to_absolute_dir_path, ProjectPath};

/// Shows project information.
pub fn project_info(
    path: &Path,
    packages_level: PackagesInfoLevel,
    recursive: bool,
) -> anyhow::Result<()> {
    if !recursive {
        return print_project_info(&ProjectPath::try_from(path)?, packages_level);
    }

    let path = to_absolute_dir_path(&path)?;
    println!("Searching for Unity projects in: {}", path.display(),);

    let mut it = recursive_dir_iter(path);
    while let Some(Ok(entry)) = it.next() {
        if let Ok(path) = ProjectPath::try_from(entry.path()) {
            println!();
            if let Err(err) = print_project_info(&path, packages_level) {
                println!("{}{}", INDENT, err.red());
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
            println!("{INDENT}Product name:  {}", ps.product_name.bold());
            println!("{INDENT}Company name:  {}", ps.company_name.bold());
            println!("{INDENT}Version:       {}", ps.bundle_version.bold());
        }

        Err(e) => {
            println!(
                "{INDENT}{}: {}",
                "No project settings found".yellow(),
                e.yellow()
            );
        }
    }

    print!(
        "{INDENT}Unity version: {} - {}",
        unity_version.bold(),
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
                "{INDENT}{}",
                "No `manifest.json` file found, no packages info available.".yellow()
            );
            Ok(())
        }
        PackagesAvailability::LockFileDisabled => {
            println!(
                "{INDENT}{}",
                "Packages lock file is disabled in `manifest.json`, no packages info available."
                    .yellow()
            );
            Ok(())
        }
        PackagesAvailability::NoLockFile => {
            println!(
                "{INDENT}{}",
                "No `packages-lock.json` file found, no packages info available.".yellow()
            );
            Ok(())
        }
        PackagesAvailability::Packages(packages) => {
            let mut packages = packages
                .dependencies
                .iter()
                .filter(|(name, package)| package_level.evaluate(name, package))
                .sorted_by(|(_, pi1), (_, pi2)| pi1.source.cmp(&pi2.source))
                .peekable();

            if packages.peek().is_none() {
                return Ok(());
            }

            println!();
            println!(
                "{} {} {}",
                "Packages:".bold(),
                package_level.bold(),
                "(L=local, E=embedded, G=git, T=tarball, R=registry, B=builtin)".bold()
            );

            for (name, package) in packages {
                println!(
                    "{INDENT}{} {} ({})",
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
