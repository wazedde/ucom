use crate::cli::{CacheAction, Cli, Command};
use crate::commands::test_cmd::run_tests;
use crate::commands::{
    INDENT, add_to_project, build_project, find_project_updates, install_latest_matching,
    list_versions, new_project, open_project, project_info, run_unity,
};
use crate::unity::release_api::FetchMode;
use anyhow::Context;
use clap::Parser;
use std::fmt::Display;
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
        .with_context(|| color_error("Cannot set cache from environment"))?;

    match command {
        Command::List {
            list_type,
            version_filter,
            force,
        } => {
            let mode = if force {
                FetchMode::Force
            } else {
                FetchMode::Auto
            };
            list_versions(list_type, version_filter.as_deref(), mode)
                .with_context(|| color_error(&format!("Cannot list `{list_type}`")).to_string())
        }

        Command::Install { version } => install_latest_matching(&version, FetchMode::Auto)
            .with_context(|| color_error("Cannot install Unity version")),

        Command::Info {
            project_dir,
            install_required,
            recursive,
            packages,
        } => project_info(
            &project_dir,
            packages,
            install_required,
            recursive,
            FetchMode::Auto,
        )
        .with_context(|| color_error("Cannot show project info")),

        Command::Check {
            project_dir,
            install_latest,
            report,
        } => find_project_updates(&project_dir, install_latest, report, FetchMode::Auto)
            .with_context(|| color_error("Cannot show Unity updates for project")),

        Command::Run(settings) => run_unity(settings).context(color_error("Cannot run Unity")),

        Command::New(settings) => {
            new_project(settings).with_context(|| color_error("Cannot create new Unity project"))
        }

        Command::Open(settings) => {
            open_project(settings).with_context(|| color_error("Cannot open Unity project"))
        }

        Command::Build(settings) => {
            build_project(&settings).with_context(|| color_error("Cannot build project"))
        }

        Command::Test(settings) => {
            run_tests(&settings).with_context(|| color_error("Cannot run tests"))
        }

        Command::Add(arguments) => {
            add_to_project(&arguments).with_context(|| color_error("Cannot add file to project"))
        }

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

fn color_error(message: &str) -> impl Display {
    message.red().bold()
}
