#![warn(rust_2018_idioms, clippy::trivially_copy_pass_by_ref)]
// #![warn(clippy::pedantic)]

use std::process::exit;

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap::Parser;
use colored::Colorize;

use crate::cli::{Action, Cli};
use crate::commands::*;

mod cli;
mod commands;
mod unity;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.disable_color {
        colored::control::set_override(false);
    } else {
        #[cfg(windows)]
        colored::control::set_virtual_terminal(true).expect("Always returns Ok()");
    }

    if cli.injected_script {
        println!("{}", UNITY_BUILD_SCRIPT);
        exit(0);
    }

    let Some(command) = cli.command else {
        let _ = Cli::command().print_help();
        exit(0)
    };

    match command {
        Action::List {
            list_type,
            version_pattern,
        } => list_versions(list_type, version_pattern.as_deref())
            .context("Cannot list installations".red().bold()),

        Action::Info {
            project_dir,
            packages,
        } => show_project_info(&project_dir, packages)
            .context("Cannot show project info".red().bold()),

        Action::UpdateCheck {
            project_dir,
            create_report: report_path,
        } => check_updates(&project_dir, report_path.as_deref())
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
    }
}
