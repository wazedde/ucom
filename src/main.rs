use std::process::exit;

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap::Parser;
use colored::Colorize;

use crate::cli::*;
use crate::commands::*;

mod build_script;
mod cli;
mod command_ext;
mod commands;
pub mod unity_project;
pub mod unity_release;
pub mod unity_version;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.injected_script {
        println!("{}", build_script::content());
        exit(0);
    }

    let Some(command) = cli.command else {
        let _result = Cli::command().print_help();
        exit(0)
    };

    match command {
        Action::List {
            version_pattern,
            check_updates,
        } => list_versions(version_pattern.as_deref(), check_updates)
            .context("Cannot list installations".red().bold()),
        Action::Info {
            project_dir,
            packages,
        } => show_project_info(&project_dir, packages)
            .context("Cannot show project info".red().bold()),
        Action::UpdateCheck { project_dir } => {
            check_unity_updates(&project_dir).context("Cannot show Unity updates for project".red().bold())
        }
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
    }
}
