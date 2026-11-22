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

/// A validated build: the command line arguments, plus the values derived from them once.
#[derive(Debug)]
pub(super) struct BuildPlan<'a> {
    pub(super) args: &'a BuildArguments,
    pub(super) output_path: PathBuf,
    pub(super) log_path: PathBuf,
    pub(super) options: BuildOptionFlags,
}

impl<'a> BuildPlan<'a> {
    /// Creates the plan for building `project` with the given arguments.
    pub(super) fn new(args: &'a BuildArguments, project: &ProjectPath) -> anyhow::Result<Self> {
        Ok(Self {
            args,
            output_path: resolve_output_path(
                args.build_path.as_deref(),
                args.output_type,
                args.target,
                project,
            )?,
            log_path: resolve_log_path(args.log_file.as_deref(), args.target, project)?,
            options: option_flags(args),
        })
    }

    /// Returns the build target as the name of `UnityEditor.BuildTarget`.
    pub(super) fn script_target(&self) -> BuildScriptTarget {
        BuildScriptTarget::from(self.args.target)
    }

    /// Returns the C# build function, if one other than the default was given.
    fn build_function(&self) -> Option<&str> {
        self.args
            .build_function
            .as_deref()
            .filter(|&f| f != DEFAULT_BUILD_FUNCTION)
    }

    /// Returns true if a build in a new Unity process uses ucom's build script: the script is
    /// injected, or the default build function is called. Editor builds always use it.
    pub(super) fn batch_uses_builder(&self) -> bool {
        self.args.inject != InjectAction::Off || self.build_function().is_none()
    }

    /// Returns the additional arguments for the Unity process.
    fn unity_args(&self) -> &[String] {
        self.args.args.as_deref().unwrap_or_default()
    }

    /// Returns true if the Unity log should be shown while building in a new Unity process.
    pub(super) fn show_log(&self) -> bool {
        !self.args.quiet
            && matches!(
                self.args.mode,
                BuildMode::Batch | BuildMode::BatchNoGraphics
            )
    }

    /// Returns the command that builds the project in a new Unity process.
    pub(super) fn batch_command(&self, project: &ProjectPath, editor_exe: &Path) -> Command {
        let mut builder = UnityCommandBuilder::new(editor_exe.to_path_buf())
            .with_project_path(project.to_path_buf())
            .with_build_target(self.args.target)
            .with_log_file(&self.log_path)
            .add_arg("-executeMethod")
            .add_arg(self.build_function().unwrap_or(DEFAULT_BUILD_FUNCTION))
            .add_arg("--ucom-build-output")
            .add_arg(&self.output_path)
            .add_arg("--ucom-build-target")
            .add_arg(self.script_target().as_ref());

        if !self.options.is_empty() {
            builder = builder
                .add_arg("--ucom-build-options")
                .add_arg(self.options.bits().to_string());
        }

        if let Some(build_args) = &self.args.build_args {
            builder = builder.add_arg("--ucom-pre-build-args").add_arg(build_args);
        }

        builder = match self.args.mode {
            BuildMode::BatchNoGraphics => builder.batch_mode(true).no_graphics(true).quit(true),
            BuildMode::Batch => builder.batch_mode(true).quit(true),
            BuildMode::EditorQuit => builder.quit(true),
            BuildMode::Editor => builder,
        };

        builder.add_args(self.unity_args()).build()
    }

    /// Returns the flags that change the build but cannot be sent to a running editor.
    pub(super) fn unsupported_in_editor(&self) -> Vec<&'static str> {
        let mut unsupported = Vec::new();
        if self.build_function().is_some() {
            unsupported.push("--build-function");
        }
        if !self.unity_args().is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command as CliCommand};
    use crate::utils::test_util::{TempDir, temp_project};
    use clap::Parser;

    /// A directory with the minimum ucom needs to recognise a Unity project.
    struct TestProject {
        _dir: TempDir,
        project: ProjectPath,
    }

    impl TestProject {
        fn new(label: &str) -> Self {
            let (dir, project) = temp_project(label);
            Self { _dir: dir, project }
        }

        /// Parses `ucom build <target> <project> <rest...>` with the real CLI and creates its plan.
        ///
        /// The project comes right after the target, since `-O` takes any number of values.
        fn plan(&self, args: &[&str]) -> anyhow::Result<BuildPlan<'static>> {
            let project = self.project.to_string_lossy().into_owned();
            let (target, rest) = args.split_first().unwrap();
            let argv = ["ucom", "build", target, project.as_str()]
                .into_iter()
                .chain(rest.iter().copied());
            let Some(CliCommand::Build(build_args)) = Cli::try_parse_from(argv).unwrap().command
            else {
                panic!("not parsed as a build command");
            };
            // Leaked so the plan can borrow the arguments for the rest of the test.
            BuildPlan::new(Box::leak(Box::new(build_args)), &self.project)
        }
    }

    #[test]
    fn batch_uses_builder_unless_not_injected_and_another_function() {
        let p = TestProject::new("plan-uses-builder");
        let uses = |args: &[&str]| p.plan(args).unwrap().batch_uses_builder();

        assert!(uses(&["macos"]));
        assert!(uses(&["macos", "-f", "My.Builder.Build"]));
        assert!(uses(&["macos", "-i", "off"]));
        assert!(uses(&["macos", "-i", "off", "-f", DEFAULT_BUILD_FUNCTION]));
        assert!(!uses(&["macos", "-i", "off", "-f", "My.Builder.Build"]));
    }

    #[test]
    fn default_paths_are_in_the_project() {
        let p = TestProject::new("plan-default-paths");
        let plan = p.plan(&["macos"]).unwrap();

        assert_eq!(
            plan.output_path,
            p.project
                .join("Builds")
                .join("Release")
                .join("OSXUniversal")
        );
        assert_eq!(
            plan.log_path,
            p.project.join("Logs").join("Build-OSXUniversal.log")
        );
    }

    #[test]
    fn output_type_and_bare_log_name_use_the_default_directories() {
        let p = TestProject::new("plan-type-log");
        let plan = p
            .plan(&["webgl", "-t", "debug", "-l", "custom.log"])
            .unwrap();

        assert_eq!(
            plan.output_path,
            p.project.join("Builds").join("Debug").join("WebGL")
        );
        assert_eq!(plan.log_path, p.project.join("Logs").join("custom.log"));
    }

    #[test]
    fn explicit_paths_are_used_as_given() {
        let p = TestProject::new("plan-explicit-paths");
        let out = p.project.join("out");
        let log = p.project.join("logs").join("build.log");
        let plan = p
            .plan(&[
                "win64",
                "-o",
                out.to_str().unwrap(),
                "-l",
                log.to_str().unwrap(),
            ])
            .unwrap();

        assert_eq!(plan.output_path, out);
        assert_eq!(plan.log_path, log);
    }

    #[test]
    fn output_path_cannot_be_the_project_directory() {
        let p = TestProject::new("plan-output-is-project");
        let project = p.project.to_string_lossy().into_owned();

        let error = p.plan(&["macos", "-o", &project]).unwrap_err().to_string();

        assert!(error.starts_with("Output directory cannot be the same as the project directory"));
    }

    #[test]
    fn option_flags_combine_shortcuts_and_build_options() {
        let p = TestProject::new("plan-options");
        let plan = p
            .plan(&["macos", "-d", "-r", "-O", "AllowDebugging", "None"])
            .unwrap();

        assert!(plan.options.contains(BuildOptions::Development));
        assert!(plan.options.contains(BuildOptions::AutoRunPlayer));
        assert!(plan.options.contains(BuildOptions::AllowDebugging));
        assert!(!plan.options.contains(BuildOptions::ShowBuiltPlayer));
        assert!(!plan.options.contains(BuildOptions::None));
        assert_eq!(plan.options.bits(), 1 | 4 | 0x200);
    }

    #[test]
    fn build_option_values_are_the_unity_names() {
        use clap::ValueEnum;

        for option in BuildOptions::value_variants() {
            let value = option.to_possible_value().unwrap();
            assert_eq!(value.get_name(), format!("{option:?}"));
        }
    }

    /// Converts a variant name to kebab case the way clap's default renaming does
    /// (`CompressWithLz4HC` to `compress-with-lz4-hc`).
    fn kebab_case(name: &str) -> String {
        let chars: Vec<char> = name.chars().collect();
        let mut kebab = String::new();
        for (i, &c) in chars.iter().enumerate() {
            if c.is_uppercase() && i > 0 {
                let previous = chars[i - 1];
                let next_is_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
                if previous.is_lowercase()
                    || previous.is_ascii_digit()
                    || (previous.is_uppercase() && next_is_lower)
                {
                    kebab.push('-');
                }
            }
            kebab.extend(c.to_lowercase());
        }
        kebab
    }

    #[test]
    fn every_build_option_accepts_its_kebab_case_name() {
        use clap::ValueEnum;

        assert_eq!(kebab_case("CompressWithLz4HC"), "compress-with-lz4-hc");
        for option in BuildOptions::value_variants() {
            let kebab = kebab_case(&format!("{option:?}"));
            assert_eq!(
                BuildOptions::from_str(&kebab, false).ok(),
                Some(*option),
                "no alias {kebab:?} for {option:?}"
            );
        }
    }

    /// Returns the arguments of `command`, without the program.
    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn path_arg(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn batch_command_for_defaults() {
        let p = TestProject::new("plan-batch-default");
        let plan = p.plan(&["macos"]).unwrap();

        let command = plan.batch_command(&p.project, Path::new("Unity"));

        assert_eq!(command.get_program(), "Unity");
        assert_eq!(
            command_args(&command),
            [
                "-projectPath",
                &path_arg(&p.project),
                "-buildTarget",
                "OSXUniversal",
                "-logFile",
                &path_arg(&plan.log_path),
                "-batchmode",
                "-quit",
                "-executeMethod",
                "Ucom.UnityBuilder.Build",
                "--ucom-build-output",
                &path_arg(&plan.output_path),
                "--ucom-build-target",
                "StandaloneOSX",
            ]
        );
    }

    #[test]
    fn batch_command_with_options_and_extra_arguments() {
        let p = TestProject::new("plan-batch-options");
        let plan = p
            .plan(&[
                "android",
                "-d",
                "-a",
                "v1.2",
                "-f",
                "My.Builder.Build",
                "-m",
                "batch-nogfx",
                "--",
                "-extraFlag",
            ])
            .unwrap();

        let command = plan.batch_command(&p.project, Path::new("Unity"));

        assert_eq!(
            command_args(&command),
            [
                "-projectPath",
                &path_arg(&p.project),
                "-buildTarget",
                "Android",
                "-logFile",
                &path_arg(&plan.log_path),
                "-batchmode",
                "-nographics",
                "-quit",
                "-executeMethod",
                "My.Builder.Build",
                "--ucom-build-output",
                &path_arg(&plan.output_path),
                "--ucom-build-target",
                "Android",
                "--ucom-build-options",
                "1",
                "--ucom-pre-build-args",
                "v1.2",
                "-extraFlag",
            ]
        );
    }

    #[test]
    fn editor_mode_does_not_add_batch_flags() {
        let p = TestProject::new("plan-editor-mode");
        let plan = p.plan(&["macos", "-m", "editor"]).unwrap();

        let args = command_args(&plan.batch_command(&p.project, Path::new("Unity")));

        assert!(!args.iter().any(|a| a == "-batchmode" || a == "-quit"));
        assert!(!plan.show_log());
    }

    #[test]
    fn default_build_has_no_flags_unsupported_in_the_editor() {
        let p = TestProject::new("plan-editor-supported");

        assert!(
            p.plan(&["macos"])
                .unwrap()
                .unsupported_in_editor()
                .is_empty()
        );
        // Passing the default build function explicitly counts as not set.
        let plan = p.plan(&["macos", "-f", DEFAULT_BUILD_FUNCTION]).unwrap();
        assert!(plan.unsupported_in_editor().is_empty());
        assert_eq!(plan.build_function(), None);
    }

    #[test]
    fn flags_unsupported_in_the_editor_are_listed() {
        let p = TestProject::new("plan-editor-unsupported");
        let plan = p
            .plan(&["macos", "-f", "My.Builder.Build", "-a", "x", "--", "-extra"])
            .unwrap();

        // `--build-args` is sent to the editor.
        assert_eq!(
            plan.unsupported_in_editor(),
            ["--build-function", "additional Unity arguments"]
        );
    }
}
