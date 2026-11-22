use crate::cli_cache::{CacheAction, CacheArguments};
use crate::commands::output::INDENT;
use crate::utils::content_cache::{delete_cache_directory, ucom_cache_dir};
use crate::utils::path_ext::PlatformConsistentPathExt;

/// Clears or lists the download cache.
pub fn manage_cache(arguments: &CacheArguments) -> anyhow::Result<()> {
    let cache_dir = ucom_cache_dir()?;
    match arguments.action {
        CacheAction::Clear => {
            delete_cache_directory(&cache_dir)?;
            println!("Cleared cache at: {}", cache_dir.normalized_display());
        }
        CacheAction::List => {
            if !cache_dir.exists() {
                println!("No cache found at: {}", cache_dir.normalized_display());
                return Ok(());
            }

            println!("Cached files at: {}", cache_dir.normalized_display());
            for file in cache_dir.read_dir()? {
                println!("{INDENT}{}", file?.file_name().to_string_lossy());
            }
        }
    }
    Ok(())
}
