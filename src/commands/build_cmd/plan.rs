//! The build to run, derived once from the command line arguments and shared by batch builds and
//! builds through an open editor.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail};
use path_absolutize::Absolutize;

use crate::cli_build::{
    BuildArguments, BuildMode, BuildOpenTarget, BuildOptions, BuildOutputType, BuildScriptTarget,
    DEFAULT_BUILD_FUNCTION, InjectAction,
};
use crate::commands::unity_command_builder::UnityCommandBuilder;
use crate::unity::ProjectPath;
use crate::utils::path_ext::PlatformConsistentPathExt;

/// Combined `UnityEditor.BuildOptions` flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct BuildOptionFlags(i32);

impl BuildOptionFlags {
    /// Returns the flags as the integer value of `UnityEditor.BuildOptions`.
    pub(super) const fn bits(self) -> i32 {
        self.0
    }

    /// Returns true if no flags are set.
    pub(super) const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true if `option` is set. `BuildOptions::None` is never set.
    pub(super) const fn contains(self, option: BuildOptions) -> bool {
        let bits = option as i32;
        bits != 0 && self.0 & bits == bits
    }
}

impl FromIterator<BuildOptions> for BuildOptionFlags {
    fn from_iter<I: IntoIterator<Item = BuildOptions>>(options: I) -> Self {
        Self(options.into_iter().fold(0, |flags, o| flags | o as i32))
    }
}

/// A validated build: what to build, where to, and how.
#[derive(Debug)]
pub(super) struct BuildPlan {
    pub(super) target: BuildOpenTarget,
    pub(super) output_path: PathBuf,
    pub(super) log_path: PathBuf,
    pub(super) options: BuildOptionFlags,
    /// The C# build function, if one other than the default was given.
    pub(super) build_function: Option<String>,
    pub(super) build_args: Option<String>,
    pub(super) unity_args: Vec<String>,
    pub(super) mode: BuildMode,
    pub(super) inject: InjectAction,
    pub(super) quiet: bool,
    pub(super) clean: bool,
    pub(super) dry_run: bool,
    pub(super) force_editor_build: bool,
}

impl BuildPlan {
    /// Creates the plan for building `project` with the given arguments.
    pub(super) fn new(args: &BuildArguments, project: &ProjectPath) -> anyhow::Result<Self> {
        Ok(Self {
            target: args.target,
            output_path: resolve_output_path(
                args.build_path.as_deref(),
                args.output_type,
                args.target,
                project,
            )?,
            log_path: resolve_log_path(args.log_file.as_deref(), args.target, project)?,
            options: option_flags(args),
            build_function: args
                .build_function
                .clone()
                .filter(|f| f != DEFAULT_BUILD_FUNCTION),
            build_args: args.build_args.clone(),
            unity_args: args.args.clone().unwrap_or_default(),
            mode: args.mode,
            inject: args.inject,
            quiet: args.quiet,
            clean: args.clean,
            dry_run: args.dry_run,
            force_editor_build: args.force_editor_build,
        })
    }

    /// Returns the build target as the name of `UnityEditor.BuildTarget`.
    pub(super) fn script_target(&self) -> BuildScriptTarget {
        BuildScriptTarget::from(self.target)
    }

    /// Returns true if the Unity log should be shown while building in a new Unity process.
    pub(super) fn show_log(&self) -> bool {
        !self.quiet && matches!(self.mode, BuildMode::Batch | BuildMode::BatchNoGraphics)
    }

    /// Returns the command that builds the project in a new Unity process.
    pub(super) fn batch_command(&self, project: &ProjectPath, editor_exe: &Path) -> Command {
        let mut builder = UnityCommandBuilder::new(editor_exe.to_path_buf())
            .with_project_path(project.to_path_buf())
            .with_build_target(self.target.as_ref())
            .with_log_file(&self.log_path)
            .add_arg("-executeMethod")
            .add_arg(
                self.build_function
                    .as_deref()
                    .unwrap_or(DEFAULT_BUILD_FUNCTION),
            )
            .add_arg("--ucom-build-output")
            .add_arg(self.output_path.to_string_lossy().to_string())
            .add_arg("--ucom-build-target")
            .add_arg(self.script_target().as_ref());

        if !self.options.is_empty() {
            builder = builder
                .add_arg("--ucom-build-options")
                .add_arg(self.options.bits().to_string());
        }

        if let Some(build_args) = &self.build_args {
            builder = builder.add_arg("--ucom-pre-build-args").add_arg(build_args);
        }

        builder = match self.mode {
            BuildMode::BatchNoGraphics => builder.batch_mode(true).no_graphics(true).quit(true),
            BuildMode::Batch => builder.batch_mode(true).quit(true),
            BuildMode::EditorQuit => builder.quit(true),
            BuildMode::Editor => builder,
        };

        builder.add_args(self.unity_args.iter().cloned()).build()
    }

    /// Returns the flags that change the build but cannot be sent to a running editor.
    pub(super) fn unsupported_in_editor(&self) -> Vec<&'static str> {
        let mut unsupported = Vec::new();
        if self.build_function.is_some() {
            unsupported.push("--build-function");
        }
        if self.build_args.is_some() {
            unsupported.push("--build-args");
        }
        if !self.unity_args.is_empty() {
            unsupported.push("additional Unity arguments");
        }
        unsupported
    }
}

/// Returns the full path to the log file.
///
/// A bare file name, including the default `Build-<target>.log`, is placed in the project's
/// `Logs` directory.
fn resolve_log_path(
    log_file: Option<&Path>,
    target: BuildOpenTarget,
    project: &ProjectPath,
) -> anyhow::Result<PathBuf> {
    let log_file = log_file.map_or_else(|| format!("Build-{target}.log").into(), ToOwned::to_owned);

    let file_name = log_file
        .file_name()
        .ok_or_else(|| anyhow!("Invalid log file name: {}", log_file.normalized_display()))?;

    let path = if log_file == file_name {
        project.join("Logs").join(file_name)
    } else {
        log_file
    };

    Ok(path.absolutize()?.to_path_buf())
}

/// Returns the output directory for the build; by default `<project>/Builds/<type>/<target>`.
fn resolve_output_path(
    build_path: Option<&Path>,
    output_type: BuildOutputType,
    target: BuildOpenTarget,
    project: &ProjectPath,
) -> anyhow::Result<PathBuf> {
    let output_dir = match build_path {
        Some(path) => path.absolutize()?.into(),
        None => project
            .join("Builds")
            .join(output_type.as_ref())
            .join(target.as_ref()),
    };

    if project.as_ref() == output_dir {
        bail!(
            "Output directory cannot be the same as the project directory: {}",
            project.normalized_display()
        );
    }
    Ok(output_dir)
}

/// Combines the shortcut flags (`--development`, `--run`, ...) with `--build-options`.
fn option_flags(args: &BuildArguments) -> BuildOptionFlags {
    let shortcuts = [
        (args.run_player, BuildOptions::AutoRunPlayer),
        (args.development_build, BuildOptions::Development),
        (args.show_built_player, BuildOptions::ShowBuiltPlayer),
        (args.allow_debugging, BuildOptions::AllowDebugging),
        (
            args.connect_with_profiler,
            BuildOptions::ConnectWithProfiler,
        ),
        (
            args.deep_profiling,
            BuildOptions::EnableDeepProfilingSupport,
        ),
        (args.connect_to_host, BuildOptions::ConnectToHost),
    ];

    shortcuts
        .into_iter()
        .filter_map(|(set, option)| set.then_some(option))
        .chain(args.build_options.iter().copied())
        .collect()
}
