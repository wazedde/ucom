use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::anyhow;
use colored::Colorize;
use path_absolutize::Absolutize;
use spinoff::{spinners, Spinner};

use crate::cli::NewArguments;
use crate::unity::*;

const GIT_IGNORE: &str = include_str!("include/unity-gitignore.txt");

/// Creates a new Unity project and optional Git repository in the given directory.
pub fn new_project(arguments: NewArguments) -> anyhow::Result<()> {
    let project_dir = arguments.project_dir.absolutize()?;

    if project_dir.exists() {
        return Err(anyhow!(
            "Directory already exists: {}",
            project_dir.absolutize()?.display()
        ));
    }

    let version = matching_available_version(arguments.version_pattern.as_deref())?;
    let editor_exe = editor_executable_path(version)?;

    let mut cmd = Command::new(editor_exe);
    cmd.arg("-createProject")
        .arg(project_dir.as_ref())
        .args(arguments.args.unwrap_or_default());

    if arguments.quit {
        cmd.arg("-quit");
    }

    if arguments.dry_run {
        println!("{}", cmd_to_string(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println!(
            "{}",
            format!(
                "Create new Unity {} project in: {}",
                version,
                project_dir.display()
            )
            .bold()
        );
    }

    if !arguments.no_git {
        git_init(project_dir)?;
    }

    match (arguments.wait, arguments.quit && !arguments.quiet) {
        (true, true) => {
            let spinner = Spinner::new(spinners::Dots, "Creating project...", None);
            wait_with_stdout(cmd)?;
            spinner.clear();
        }
        (true, false) => wait_with_stdout(cmd)?,
        (false, _) => spawn_and_forget(cmd)?,
    }

    Ok(())
}

/// Initializes a new git repository with a default Unity specific .gitignore.
fn git_init<P: AsRef<Path>>(project_dir: P) -> anyhow::Result<()> {
    let project_dir = project_dir.as_ref();
    Command::new("git")
        .arg("init")
        .arg(project_dir)
        .output()
        .map_err(|_| anyhow!("Could not create git repository. Make sure git is available or add the --no-git flag."))?;

    let mut file = File::create(project_dir.join(".gitignore"))?;
    write!(file, "{GIT_IGNORE}").map_err(Into::into)
}
