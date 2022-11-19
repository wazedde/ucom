/// Unity Commander, a command line interface for Unity projects.
#[derive(clap::Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    #[command(subcommand)]
    pub action: Action,
}

#[derive(clap::Subcommand, Debug)]
pub enum Action {
    #[command(verbatim_doc_comment)]
    /// This command will show a list of the installed Unity versions.
    List {
        /// The Unity versions to list. You can specify a partial version; e.g. 2021 will list all
        /// the 2021.x.y versions you have installed on your system.
        #[arg(
            short = 'u',
            long = "unity",
            value_name = "VERSION",
            verbatim_doc_comment
        )]
        version_pattern: Option<String>,
    },
    /// This command will run Unity.
    /// Unless specified otherwise, the latest installed Unity version is used.
    #[command(
        visible_alias = "r",
        verbatim_doc_comment,
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    Run {
        /// The Unity version to run. You can specify a partial version; e.g. 2021 will match the
        /// latest 2021.x.y version you have installed on your system.
        #[arg(
            short = 'u',
            long = "unity",
            value_name = "VERSION",
            verbatim_doc_comment
        )]
        version_pattern: Option<String>,

        /// Waits for the command to finish before continuing.
        #[clap(long = "wait", short = 'w', action, verbatim_doc_comment)]
        wait: bool,

        /// Do not print ucom log messages.
        #[clap(long = "quiet", short = 'q', action, verbatim_doc_comment)]
        quiet: bool,

        /// Show what would be run, but do not actually run it.
        #[clap(long = "dry-run", short = 'n', action, verbatim_doc_comment)]
        dry_run: bool,

        /// A list of arguments passed directly to Unity.
        #[arg(value_name = "UNITY_ARGS", required = true, verbatim_doc_comment)]
        args: Option<Vec<String>>,
    },
    /// This command will create a new Unity project in the given directory.
    /// Unless specified otherwise, the latest installed Unity version is used.
    #[command(
        verbatim_doc_comment,
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
    New {
        /// The Unity version to use for the new project. You can specify a partial version;
        /// e.g. 2021 will match the latest 2021.x.y version you have installed on your system.
        #[arg(
            short = 'u',
            long = "unity",
            value_name = "VERSION",
            verbatim_doc_comment
        )]
        version_pattern: Option<String>,

        /// The directory where the project is created. This directory should not exist yet.
        #[arg(
            required = true,
            verbatim_doc_comment,
            value_name = "DIR",
            value_hint = clap::ValueHint::DirPath
        )]
        path: std::path::PathBuf,

        /// Waits for the command to finish before continuing.
        #[clap(long = "wait", short = 'w', action, verbatim_doc_comment)]
        wait: bool,

        /// Do not print ucom log messages.
        #[clap(long = "quiet", short = 'q', action, verbatim_doc_comment)]
        quiet: bool,

        /// Show what would be run, but do not actually run it.
        #[clap(long = "dry-run", short = 'n', action, verbatim_doc_comment)]
        dry_run: bool,

        /// A list of arguments passed directly to Unity.
        #[arg(value_name = "UNITY_ARGS", verbatim_doc_comment)]
        args: Option<Vec<String>>,
    },
    /// This command will open the Unity project in the given directory.
    #[command(
        visible_alias = "o",
        trailing_var_arg = true,
        allow_hyphen_values = true,
        verbatim_doc_comment
    )]
    Open {
        /// The directory of the project.
        #[arg(value_name = "DIR", value_hint = clap::ValueHint::DirPath, verbatim_doc_comment)]
        path: std::path::PathBuf,

        /// The Unity version to open the project with. Use it to open a project with a newer
        /// Unity version. You can specify a partial version; e.g. 2021 will match the latest
        /// 2021.x.y version you have installed on your system.
        #[arg(
            short = 'u',
            long = "unity",
            value_name = "VERSION",
            verbatim_doc_comment
        )]
        version_pattern: Option<String>,

        /// Waits for the command to finish before continuing.
        #[clap(long = "wait", short = 'w', action, verbatim_doc_comment)]
        wait: bool,

        /// Do not print ucom log messages.
        #[clap(long = "quiet", short = 'q', action, verbatim_doc_comment)]
        quiet: bool,

        /// Show what would be run, but do not actually run it.
        #[clap(long = "dry-run", short = 'n', action, verbatim_doc_comment)]
        dry_run: bool,

        /// A list of arguments passed directly to Unity.
        #[arg(value_name = "UNITY_ARGS", verbatim_doc_comment)]
        args: Option<Vec<String>>,
    },
}
