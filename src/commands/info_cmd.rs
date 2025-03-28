use crate::cli::PackagesInfoLevel;
use crate::commands::{
    INDENT, MARK_AVAILABLE, MARK_UNAVAILABLE, install_latest_matching, println_bold,
};
use crate::unity::project::ProjectPath;
use crate::unity::project::{
    PackageInfo, PackageSource, Packages, PackagesAvailability, ProjectSettings,
    walk_visible_directories,
};
use crate::unity::release_api::FetchMode;
use crate::unity::{Version, release_notes_url};
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::{unity, utils};
use itertools::Itertools;
use std::path::Path;
use unity::project::BuildProfilesStatus;
use yansi::Paint;

/// Shows project information.
pub fn project_info(
    path: &Path,
    packages_level: PackagesInfoLevel,
    install_required: bool,
    recursive: bool,
    mode: FetchMode,
) -> anyhow::Result<()> {
    if recursive {
        show_recursive_project_info(path, packages_level)
    } else {
        show_project_info(path, packages_level, install_required, mode)
    }
}

fn show_recursive_project_info(
    path: &Path,
    packages_level: PackagesInfoLevel,
) -> anyhow::Result<()> {
    let path = utils::resolve_absolute_dir_path(&path)?;
    println!(
        "Searching for Unity projects in: {}",
        path.normalized_display()
    );

    let mut directories = walk_visible_directories(path, 5);
    while let Some(Ok(entry)) = directories.next() {
        if let Ok(path) = ProjectPath::try_from(entry.path()) {
            println!();
            if let Err(err) = print_project_info(&path, packages_level) {
                println!("{}{}", INDENT, err.red());
            }
            directories.skip_current_dir();
        }
    }
    Ok(())
}

fn show_project_info(
    path: &Path,
    packages_level: PackagesInfoLevel,
    install_required: bool,
    mode: FetchMode,
) -> anyhow::Result<()> {
    let version = print_project_info(&ProjectPath::try_from(path)?, packages_level)?;

    if !version.is_editor_installed()? {
        println!();
        if install_required {
            install_latest_matching(version.to_interned_str(), mode)?;
        } else {
            println!(
                "Use the `{}` flag to install Unity version {}",
                "--install-required".bold(),
                version.bold()
            );
        }
    }

    Ok(())
}

fn print_project_info(
    project: &ProjectPath,
    packages_level: PackagesInfoLevel,
) -> anyhow::Result<Version> {
    let unity_version = project.unity_version()?;
    println_bold!("Project info for: {}", project.normalized_display());

    match ProjectSettings::from_project(project) {
        Ok(ps) => {
            println!("{INDENT}Product name:  {}", ps.product_name.bold());
            println!("{INDENT}Company name:  {}", ps.company_name.bold());
            println!("{INDENT}Version:       {}", ps.bundle_version.bold());
        }

        Err(e) => {
            println!(
                "{INDENT}{}: {}",
                "Could not read project settings".yellow(),
                e.yellow()
            );
        }
    }

    let installed_marker = if unity_version.is_editor_installed()? {
        MARK_AVAILABLE.green().bold()
    } else {
        MARK_UNAVAILABLE.red().bold()
    };

    print!(
        "{}{}Unity version: {} - {}",
        installed_marker,
        " ".repeat(INDENT.len() - 1),
        unity_version.bold(),
        release_notes_url(unity_version).bright_blue()
    );

    let installed = unity_version.is_editor_installed()?;

    if installed {
        println!(" {}", "(installed)".green().bold());
    } else {
        println!(" {}", "(not installed)".red().bold());
    }

    // Print the available build profiles
    let build_profiles = project.build_profiles(unity_version)?;
    if let BuildProfilesStatus::Available(profiles) = build_profiles {
        println!();
        println_bold!("Build profiles:");
        for profile in profiles {
            println!("{INDENT}{}", profile.normalized_display());
        }
    }

    if packages_level != PackagesInfoLevel::None {
        print_project_packages(project, packages_level)?;
    };

    Ok(unity_version)
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
                .filter(|(name, package)| package_level.is_allowed(name, package))
                .sorted_unstable_by(|(_, pi1), (_, pi2)| pi1.source.cmp(&pi2.source))
                .peekable();

            if packages.peek().is_none() {
                return Ok(());
            }

            println!();
            println_bold!(
                "Packages: {} (L=local, E=embedded, G=git, T=tarball, R=registry, B=builtin)",
                package_level,
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
    fn is_allowed(self, name: &str, package: &PackageInfo) -> bool {
        match self {
            Self::None => false,

            Self::ExcludingUnity => {
                package.depth == 0
                    && package.source.is_some_and(|ps| {
                        ps < PackageSource::Registry
                            || (ps == PackageSource::Registry
                                && package
                                    .url
                                    .as_ref()
                                    .is_some_and(|u| u != "https://packages.unity.com"))
                    })
            }

            Self::IncludingUnity => {
                package.depth == 0
                    && package.source.is_some_and(|ps| {
                        ps < PackageSource::Builtin || name.starts_with("com.unity.feature.")
                    })
            }

            Self::All => true,
        }
    }
}
