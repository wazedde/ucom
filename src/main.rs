use crate::cli::{CacheAction, Cli, Command};
use crate::commands::test_cmd::run_tests;
use crate::commands::{
    INDENT, add_to_project, build_project, find_project_updates, install_latest_matching,
    list_versions, new_project, open_project, project_info, run_unity,
};
use crate::style_definitions::ERROR;
use crate::unity::release_api::UpdatePolicy;
use anyhow::Context;
use clap::Parser;
use std::io::IsTerminal;
use utils::content_cache::{
    configure_cache_from_environment, delete_cache_directory, ucom_cache_dir,
};
use yansi::Paint;

mod cli;
mod cli_add;
mod cli_build;
mod cli_new;
mod cli_run;
mod cli_test;
mod commands;
mod nunit;
mod style_definitions;
mod unity;
mod utils;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let Some(command) = cli.command else {
        return Ok(());
    };

    if cli.no_color || !std::io::stdout().is_terminal() {
        yansi::disable();
    }

    configure_cache_from_environment()
        .with_context(|| "Cannot set cache from environment".paint(ERROR))?;

    match command {
        Command::List {
            list_type,
            version_filter,
            force,
        } => {
            let mode = if force {
                UpdatePolicy::ForceRefresh
            } else {
                UpdatePolicy::Incremental
            };
            list_versions(list_type, version_filter.as_deref(), mode).with_context(|| {
                format!("Cannot list `{list_type}`")
                    .paint(ERROR)
                    .to_string()
            })
        }

        Command::Install { version } => {
            install_latest_matching(&version, UpdatePolicy::Incremental)
                .with_context(|| "Cannot install the Unity version".paint(ERROR))
        }

        Command::Info {
            project_dir,
            install_required,
            recursive,
            packages,
            report,
        } => project_info(
            &project_dir,
            packages,
            install_required,
            recursive,
            report,
            UpdatePolicy::Incremental,
        )
        .with_context(|| "Cannot show project info".paint(ERROR)),

        Command::Updates {
            project_dir,
            install_latest,
            report,
        } => find_project_updates(
            &project_dir,
            install_latest,
            report,
            UpdatePolicy::Incremental,
        )
        .with_context(|| "Cannot show Unity updates for the project".paint(ERROR)),

        Command::Run(settings) => run_unity(settings).context("Cannot run Unity".paint(ERROR)),

        Command::New(settings) => new_project(settings)
            .with_context(|| "Cannot create the new Unity project".paint(ERROR)),

        Command::Open(settings) => {
            open_project(settings).with_context(|| "Cannot open the Unity project".paint(ERROR))
        }

        Command::Build(settings) => {
            build_project(&settings).with_context(|| "Cannot build the project".paint(ERROR))
        }

        Command::Test(settings) => {
            run_tests(&settings).with_context(|| "Cannot run tests".paint(ERROR))
        }

        Command::Add(arguments) => add_to_project(&arguments)
            .with_context(|| "Cannot add the file to the project".paint(ERROR)),

        Command::Cache { action: command } => {
            match command {
                CacheAction::Clear => {
                    delete_cache_directory();
                    println!("Cleared cache at: {}", ucom_cache_dir()?.display());
                }
                CacheAction::List => {
                    let cache_dir = ucom_cache_dir()?;
                    if !cache_dir.exists() {
                        println!("No cache found at: {}", cache_dir.display());
                        return Ok(());
                    }

                    println!("Cached files at: {}", cache_dir.display());
                    for file in ucom_cache_dir()?.read_dir()? {
                        println!("{}{}", INDENT, file?.file_name().to_string_lossy());
                    }
                }
            }
            Ok(())
        }
    }
}
