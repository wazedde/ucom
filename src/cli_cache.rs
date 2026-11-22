use clap::{Args, ValueEnum};

#[derive(Args)]
pub struct CacheArguments {
    /// Action to perform on the cache.
    #[arg(value_enum)]
    pub action: CacheAction,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum CacheAction {
    /// Remove all cached download files.
    Clear,
    /// Display a list of currently cached files.
    List,
}
