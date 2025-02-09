use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context};
use path_absolutize::Absolutize;

use crate::cli_add::IncludedFile;
use crate::cli_new::NewArguments;
use crate::commands::term_stat::TermStat;
use crate::commands::{add_file_to_project, println_b, INDENT, PERSISTENT_BUILD_SCRIPT_ROOT};
use crate::unity::installed::Installations;
use crate::unity::*;

/// Creates a new Unity project and optional Git repository in the given directory.
pub(crate) fn new_project(arguments: NewArguments) -> anyhow::Result<()> {
    let project_dir = arguments.project_dir.absolutize()?;

    if project_dir.exists() {
        return Err(anyhow!(
            "Directory already exists: {}",
            project_dir.display()
        ));
    }

    let version = Installations::latest(Some(&arguments.version_pattern))?;
    let editor_exe = version.editor_executable_path()?;

    let mut cmd = Command::new(editor_exe);
    cmd.arg("-createProject")
        .arg(project_dir.as_ref())
        .args(arguments.args.unwrap_or_default());

    if arguments.quit {
        cmd.arg("-quit");
    }

    if let Some(target) = arguments.target {
        cmd.args(["-buildTarget", target.as_ref()]);
    }

    if arguments.dry_run {
        println!("{}", build_command_line(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println_b!(
            "Create new Unity {} project in: {}",
            version,
            project_dir.display()
        );
    }

    if arguments.add_builder_menu {
        let parent_dir = &PathBuf::from(PERSISTENT_BUILD_SCRIPT_ROOT);

        print!("{}", INDENT);
        add_file_to_project(&project_dir, parent_dir, IncludedFile::Builder)?;
        print!("{}", INDENT);
        add_file_to_project(&project_dir, parent_dir, IncludedFile::BuilderMenu)?;
    }

    if !arguments.no_git {
        git_init(project_dir, arguments.include_lfs)?;
    }

    match (arguments.wait, arguments.quit && !arguments.quiet) {
        (true, true) => {
            let _status = TermStat::new("Creating", "project...");
            wait_with_stdout(cmd)?;
        }
        (true, false) => wait_with_stdout(cmd)?,
        (false, _) => spawn_and_forget(cmd)?,
    }

    Ok(())
}

/// Initializes a new git repository with a default Unity specific .gitignore.
fn git_init(project_dir: impl AsRef<Path>, include_lfs: bool) -> anyhow::Result<()> {
    println!("Initializing Git repository:");
    let project_dir = project_dir.as_ref();

    let init_context =
        "Could not create Git repository. Make sure Git is available or add the --no-git flag.";
    let output = Command::new("git")
        .arg("init")
        .arg(project_dir)
        .output()
        .context(init_context)?;

    if !output.status.success() {
        return Err(anyhow!("{}", String::from_utf8_lossy(&output.stderr))).context(init_context);
    }

    print!("{}", INDENT);
    add_file_to_project(project_dir, PathBuf::default(), IncludedFile::GitIgnore)?;

    if include_lfs {
        println!("Initializing Git LFS:");
        env::set_current_dir(project_dir)?;

        let lfs_context =
            "Could not initialize Git LFS. Make sure LFS is available or don't add the --lfs flag.";
        let output = Command::new("git")
            .arg("lfs")
            .arg("install")
            .output()
            .context(lfs_context)?;

        if !output.status.success() {
            return Err(anyhow!("{}", String::from_utf8_lossy(&output.stderr)))
                .context(lfs_context);
        }

        print!("{}", INDENT);
        add_file_to_project(project_dir, PathBuf::default(), IncludedFile::GitAttributes)?;
    }
    Ok(())
}
