#![warn(rust_2018_idioms, clippy::trivially_copy_pass_by_ref)]

use std::process::exit;

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap::Parser;
use colored::Colorize;

use crate::cli::{Action, Cli};
use crate::cmd_build::build_project;
use crate::cmd_check_updates::check_unity_updates;
use crate::cmd_list::list_versions;
use crate::cmd_new::new_project;
use crate::cmd_open::*;
use crate::cmd_project_info::show_project_info;
use crate::cmd_run::run_unity;

mod build_script;
mod cli;
mod cmd_build;
mod cmd_check_updates;
mod cmd_list;
mod cmd_new;
mod cmd_open;
mod cmd_project_info;
mod cmd_run;
mod unity_cmd;
mod unity_project;
mod unity_release;
mod unity_version;

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.disable_color {
        colored::control::set_override(false);
    }

    if cli.injected_script {
        println!("{}", build_script::content());
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
        } => check_unity_updates(&project_dir, report_path.as_deref())
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
