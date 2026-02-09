use anstream::println;
use bytesize::ByteSize;
use clap::{Args, Subcommand};
use owo_colors::OwoColorize;
use rv_cache::CleanReporter;

use crate::{GlobalArgs, config::Config};

#[derive(Args)]
pub struct CacheCommandArgs {
    #[command(subcommand)]
    pub command: CacheCommand,
}

#[derive(Subcommand)]
pub enum CacheCommand {
    #[command(about = "Clear the cache")]
    Clean,
    #[command(about = "Prune all unused entries from the cache")]
    Prune,
    #[command(about = "Show the cache directory")]
    Dir,
}
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Config(#[from] crate::config::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) fn cache(global_args: &GlobalArgs, args: CacheCommandArgs) -> Result<()> {
    let config = &Config::new(global_args, None)?;

    match args.command {
        CacheCommand::Dir => cache_dir(config)?,
        CacheCommand::Clean => cache_clean(config)?,
        CacheCommand::Prune => cache_prune(config)?,
    };

    Ok(())
}

fn cache_dir(config: &Config) -> Result<()> {
    println!("{}", config.cache.root().as_str().cyan());
    Ok(())
}
fn cache_clean(config: &Config) -> Result<()> {
    struct Reporter {}
    impl CleanReporter for Reporter {
        fn on_clean(&self) {}
        fn on_complete(&self) {}
    }
    let removal = config.cache.clear(Box::new(Reporter {}))?;
    let num_bytes_cleaned = ByteSize::b(removal.bytes).display().iec_short();
    println!(
        "Removed {} directories, totalling {}",
        removal.dirs.cyan(),
        num_bytes_cleaned.cyan()
    );
    Ok(())
}

fn cache_prune(config: &Config) -> Result<()> {
    let removal = config.cache.prune()?;
    let num_bytes_cleaned = ByteSize::b(removal.bytes).display().iec_short();
    println!(
        "Removed {} directories, totalling {}",
        removal.dirs.cyan(),
        num_bytes_cleaned.cyan()
    );
    Ok(())
}
