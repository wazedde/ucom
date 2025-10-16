use crate::cli::PackagesInfoLevel;
use crate::commands::{MARK_AVAILABLE, MARK_UNAVAILABLE, install_latest_matching};
use crate::style_definitions::*;
use crate::unity::project::ProjectPath;
use crate::unity::project::{
    PackageInfo, PackageSource, Packages, PackagesAvailability, ProjectSettings,
    walk_visible_directories,
};
use crate::unity::release_api::{UpdatePolicy, fetch_latest_releases};
use crate::unity::{BuildProfilesStatus, Version, release_notes_url};
use crate::utils;
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::report::{HeaderLevel, Report};
use content_cache::RemoteChangeCheck;
use itertools::Itertools;
use std::path::Path;
use utils::content_cache;
use yansi::Paint;

/// Shows project information.
pub fn project_info(
    path: &Path,
    packages_level: PackagesInfoLevel,
    install_required: bool,
    recursive: bool,
    report: bool,
    mode: UpdatePolicy,
) -> anyhow::Result<()> {
    if recursive {
        show_recursive_project_info(path, packages_level, report)
    } else {
        show_project_info(path, packages_level, report, install_required, mode)
    }
}

fn show_recursive_project_info(
    path: &Path,
    packages_level: PackagesInfoLevel,
    show_release_notes: bool,
) -> anyhow::Result<()> {
    let report = if show_release_notes {
        yansi::disable();
        Report::Markdown
    } else {
        Report::Terminal
    };

    let path = utils::resolve_absolute_dir_path(&path)?;
    report.paragraph(format_args!(
        "Searching for Unity projects in: {}",
        path.normalized_display()
    ));

    let mut directories = walk_visible_directories(path, 5);
    while let Some(Ok(entry)) = directories.next() {
        if let Ok(path) = ProjectPath::try_from(entry.path()) {
            report.blank_line();
            if let Err(err) = print_project_info(&path, packages_level, &report, show_release_notes)
            {
                report.list_item(format_args!("{} {}", "Error:".paint(ERROR), err));
            }
            directories.skip_current_dir();
        }
    }
    Ok(())
}

fn show_project_info(
    path: &Path,
    packages_level: PackagesInfoLevel,
    show_release_notes: bool,
    install_required: bool,
    mode: UpdatePolicy,
) -> anyhow::Result<()> {
    let report = if show_release_notes {
        yansi::disable();
        Report::Markdown
    } else {
        Report::Terminal
    };
    let version = print_project_info(
        &ProjectPath::try_from(path)?,
        packages_level,
        &report,
        show_release_notes,
    )?;

    if !version.is_editor_installed()? {
        report.blank_line();
        if install_required {
            install_latest_matching(version.to_interned_str(), mode)?;
        } else {
            report.paragraph(format_args!(
                "Use the `{}` flag to install Unity version {}",
                "--install-required".bold(),
                version.bold()
            ));
        }
    }

    Ok(())
}

fn print_project_info(
    project: &ProjectPath,
    packages_level: PackagesInfoLevel,
    report: &Report,
    show_release_notes: bool,
) -> anyhow::Result<Version> {
    let unity_version = project.unity_version()?;
    report.header(
        format_args!("Project info for: {}", project.normalized_display()),
        HeaderLevel::H2,
    );

    match ProjectSettings::from_project(project) {
        Ok(ps) => {
            report.list_item(format_args!("Product name:  {}", ps.product_name.bold()));
            report.list_item(format_args!("Company name:  {}", ps.company_name.bold()));
            report.list_item(format_args!("Version:       {}", ps.bundle_version.bold()));
        }

        Err(e) => {
            report.list_item(format_args!(
                "{}: {}",
                "Could not read project settings".paint(WARNING),
                e.paint(WARNING),
            ));
        }
    }

    let is_installed = unity_version.is_editor_installed()?;

    report.marked_item(
        format_args!(
            "Unity version: {} - {} ({})",
            unity_version.bold(),
            release_notes_url(unity_version).paint(LINK),
            if is_installed {
                "installed"
            } else {
                "not installed"
            }
        ),
        if is_installed {
            MARK_AVAILABLE.paint(OK).bold()
        } else {
            MARK_UNAVAILABLE.paint(ERROR).bold()
        },
    );

    // Print the available build profiles
    let build_profiles = project.build_profiles(unity_version)?;
    if let BuildProfilesStatus::Available(profiles) = build_profiles {
        report.blank_line();
        report.header("Build profiles:", HeaderLevel::H2);
        for profile in profiles {
            report.list_item(profile.normalized_display());
        }
    }

    if packages_level != PackagesInfoLevel::None {
        print_project_packages(project, packages_level, report)?;
    }

    if show_release_notes {
        let releases = fetch_latest_releases(UpdatePolicy::Incremental)?;
        let release = releases.get_by_version(unity_version)?;

        let url = &release.release_notes.url;
        let body = content_cache::fetch_content(url, RemoteChangeCheck::Validate)?;

        report.blank_line();
        report.header(
            format_args!("Release notes for [{}]({url})", release.version),
            HeaderLevel::H2,
        );
        report.paragraph(body);
    }
    Ok(unity_version)
}

/// Show packages used by the project.
fn print_project_packages(
    project: &ProjectPath,
    package_level: PackagesInfoLevel,
    report: &Report,
) -> anyhow::Result<()> {
    let availability = Packages::from_project(project)?;

    match availability {
        PackagesAvailability::NoManifest => {
            report.list_item(
                "No `manifest.json` file found, no packages info available.".paint(WARNING),
            );
            Ok(())
        }
        PackagesAvailability::LockFileDisabled => {
            report.list_item(
                "Packages lock file is disabled in `manifest.json`, no packages info available."
                    .paint(WARNING),
            );
            Ok(())
        }
        PackagesAvailability::NoLockFile => {
            report.list_item(
                "No `packages-lock.json` file found, no packages info available.".paint(WARNING),
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

            report.blank_line();
            report.header(
                format_args!(
                    "Packages: {package_level} (L=local, E=embedded, G=git, T=tarball, R=registry, B=builtin)",
                ),
                HeaderLevel::H2,
            );

            for (name, package) in packages {
                report.list_item(format_args!(
                    "{} {} ({})",
                    package.source.as_ref().map_or("?", |s| s.to_short_str()),
                    name,
                    package.version,
                ));
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
