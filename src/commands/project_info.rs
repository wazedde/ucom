use std::path::Path;

use colored::Colorize;

use crate::cli::PackagesInfoLevel;
use crate::unity::release_notes_url;
use crate::unity::unity_project::*;

/// Shows project information.
pub fn print_project_info(
    project_dir: &Path,
    packages_level: PackagesInfoLevel,
) -> anyhow::Result<()> {
    let project_dir = validate_project_path(&project_dir)?;
    let version = version_used_by_project(&project_dir)?;

    println!(
        "{}",
        format!("Project info for: {}", project_dir.display()).bold()
    );

    if let Ok(settings) = ProjectSettings::from_project(&project_dir) {
        let ps = settings.player_settings;
        println!("    Product Name:  {}", ps.product_name.bold());
        println!("    Company Name:  {}", ps.company_name.bold());
        println!("    Version:       {}", ps.bundle_version.bold());
    }

    print!(
        "    Unity Version: {} - {}",
        version.to_string().bold(),
        release_notes_url(version).bright_blue().underline()
    );

    if is_editor_installed(version)? {
        println!();
    } else {
        println!(" {}", "*not installed".red().bold());
    }

    if packages_level != PackagesInfoLevel::Lev0 {
        print_project_packages(project_dir.as_ref(), packages_level)?;
    };

    Ok(())
}

/// Show packages used by the project.
fn print_project_packages(
    project_dir: &Path,
    package_level: PackagesInfoLevel,
) -> anyhow::Result<()> {
    let packages = Packages::from_project(project_dir)?;

    let mut packages: Vec<_> = packages
        .dependencies
        .iter()
        .filter(|(_, package)| package_level.eval(package))
        .collect();

    packages.sort_by(|(_, pi1), (_, pi2)| pi1.source.cmp(&pi2.source));

    if packages.is_empty() {
        return Ok(());
    }

    println!();

    let (enabled, disabled) = match package_level {
        PackagesInfoLevel::Lev0 => ("", ", L=local, E=embedded, G=git"),
        PackagesInfoLevel::Lev1 => (", L=local, E=embedded, G=git", ", R=registry, B=builtin"),
        PackagesInfoLevel::Lev2 => (", L=local, E=embedded, G=git, R=registry", ", B=builtin"),
        PackagesInfoLevel::Lev3 => (", L=local, E=embedded, G=git, R=registry, B=builtin", ""),
    };

    let line = format!(
        "Packages (Level={}{}{})",
        package_level,
        enabled,
        disabled.bright_black()
    );

    println!("{}", line.bold());

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

impl PackagesInfoLevel {
    // Evaluates if the PackageInfo is allowed by the info level.
    fn eval(self, package: &PackageInfo) -> bool {
        match self {
            Self::Lev0 => false,
            Self::Lev1 => {
                package.depth == 0
                    && (package.source == Some(PackageSource::Git)
                        || package.source == Some(PackageSource::Embedded)
                        || package.source == Some(PackageSource::Local))
            }
            Self::Lev2 => {
                package.depth == 0
                    && (package.source == Some(PackageSource::Git)
                        || package.source == Some(PackageSource::Embedded)
                        || package.source == Some(PackageSource::Local)
                        || package.source == Some(PackageSource::Registry))
            }
            Self::Lev3 => true,
        }
    }
}
