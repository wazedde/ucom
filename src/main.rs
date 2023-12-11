use std::process::exit;

use anyhow::Context;
use clap::CommandFactory;
use clap::Parser;
use colored::Colorize;

use crate::cli::{Action, CacheAction, Cli};
use crate::commands::*;
use crate::unity::http_cache;

mod cli;
mod commands;
mod unity;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.disable_color {
        colored::control::set_override(false);
    } else {
        #[cfg(windows)]
        colored::control::set_virtual_terminal(true).expect("Always returns Ok()");
    }

    let Some(command) = cli.command else {
        _ = Cli::command().print_help();
        exit(0)
    };

    http_cache::set_cache_from_env();

    match command {
        Action::List {
            list_type,
            version_pattern,
        } => list_versions(list_type, version_pattern.as_deref())
            .context("Cannot list installations".red().bold()),

        Action::Info {
            project_dir,
            recursive,
            packages,
        } => project_info(&project_dir, packages, recursive)
            .context("Cannot show project info".red().bold()),

        Action::Check {
            project_dir,
            report: report_path,
        } => check_updates(&project_dir, report_path)
            .context("Cannot show Unity updates for project".red().bold()),

        Action::Run(settings) => run_unity(settings).context("Cannot run Unity".red().bold()),

        Action::New(settings) => {
            new_project(settings).context("Cannot create new Unity project".red().bold())
        }

        Action::Open(settings) => {
            open_project(settings).context("Cannot open Unity project".red().bold())
        }

        Action::Build(settings) => {
            build_project(settings).context("Cannot build project".red().bold())
        }

        Action::Template { template } => {
            println!("{}", template.data().fetch_content()?);
            Ok(())
        }
        Action::Cache { action: command } => {
            match command {
                CacheAction::Clear => {
                    http_cache::clear();
                    println!(
                        "Cleared cache at: {}",
                        http_cache::ucom_cache_dir().display()
                    );
                }
                CacheAction::Show => {
                    let cache_dir = http_cache::ucom_cache_dir();

                    if !cache_dir.exists() {
                        println!("No cache found at: {}", cache_dir.display());
                        return Ok(());
                    }

                    println!("Cached files at: {}", cache_dir.display());
                    for file in http_cache::ucom_cache_dir().read_dir()? {
                        println!("    {}", file?.file_name().to_string_lossy());
                    }
                }
            }
            Ok(())
        }
    }
}
