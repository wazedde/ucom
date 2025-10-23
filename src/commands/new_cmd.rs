use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, anyhow};
use path_absolutize::Absolutize;

use crate::cli_add::UnityTemplateFile;
use crate::cli_new::NewArguments;
use crate::commands::{
    PERSISTENT_BUILD_SCRIPT_ROOT, UnityCommandBuilder, add_file_to_project, check_version_issues,
    execute_unity_command,
};
use crate::unity::build_command_line;
use crate::unity::installations::Installations;
use crate::utils::path_ext::PlatformConsistentPathExt;
use crate::utils::status_line::StatusLine;

/// Version of `println!` that writes bold text.
macro_rules! println_bold {
    ($($arg:tt)*) => {
        println!("{}", yansi::Paint::new(format_args!($($arg)*)).bold());
    };
}

/// Creates a new Unity project and optional Git repository in the given directory.
pub fn new_project(arguments: NewArguments) -> anyhow::Result<()> {
    let project_dir = arguments.project_dir.absolutize()?;

    if project_dir.exists() {
        return Err(anyhow!(
            "Directory already exists: {}",
            project_dir.normalized_display()
        ));
    }

    let version = Installations::latest_installed_version(Some(&arguments.version_pattern))?;
    let editor_exe = version.editor_executable_path()?;

    let mut builder = UnityCommandBuilder::new(editor_exe)
        .add_arg("-createProject")
        .add_arg(project_dir.to_string_lossy().to_string())
        .add_args(arguments.args.unwrap_or_default())
        .quit(arguments.quit);

    if let Some(target) = arguments.target {
        builder = builder.with_build_target(target.as_ref());
    }

    let cmd = builder.build();

    if arguments.dry_run {
        println!("{}", build_command_line(&cmd));
        return Ok(());
    }

    if !arguments.quiet {
        println_bold!(
            "Create new Unity {v} project in: {p}",
            v = version,
            p = project_dir.normalized_display()
        );
        check_version_issues(version);
    }

    if arguments.add_builder_menu {
        let parent_dir = &PathBuf::from(PERSISTENT_BUILD_SCRIPT_ROOT);
        add_file_to_project(&project_dir, parent_dir, UnityTemplateFile::Builder)?;
        add_file_to_project(&project_dir, parent_dir, UnityTemplateFile::BuilderMenu)?;
    }

    if !arguments.no_git {
        git_init(project_dir, arguments.include_lfs)?;
    }

    let _status = if arguments.wait && arguments.quit && !arguments.quiet {
        StatusLine::new("Creating", "project...")
    } else {
        StatusLine::new_silent()
    };

    execute_unity_command(cmd, arguments.wait, arguments.quiet)?;

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

    add_file_to_project(
        project_dir,
        PathBuf::default(),
        UnityTemplateFile::GitIgnore,
    )?;

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

        add_file_to_project(
            project_dir,
            PathBuf::default(),
            UnityTemplateFile::GitAttributes,
        )?;
    }
    Ok(())
}
