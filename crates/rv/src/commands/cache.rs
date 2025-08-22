use std::io;

use anstream::println;
use bytesize::ByteSize;
use clap::{Args, Subcommand};
use owo_colors::OwoColorize;
use rv_cache::CleanReporter;
use tracing::info;

use crate::config::Config;

#[derive(Args)]
pub struct CacheCommandArgs {
    #[command(subcommand)]
    pub command: CacheCommand,
}

#[derive(Subcommand)]
pub enum CacheCommand {
    #[command(about = "Clear the cache")]
    Clean,
    #[command(about = "Show the cache directory")]
    Dir,
}

pub fn cache_dir(config: &Config) -> io::Result<()> {
    println!("{}", config.cache.root().as_str().cyan());
    Ok(())
}
pub fn cache_clean(config: &Config) -> io::Result<()> {
    struct Reporter {}
    impl CleanReporter for Reporter {
        fn on_clean(&self) {}
        fn on_complete(&self) {}
    }
    let removal = config.cache.clear(Box::new(Reporter {}))?;
    info!(
        "Removed {} directories, totalling {}",
        removal.dirs.default_color().cyan(),
        ByteSize::b(removal.bytes).display().iec_short().cyan()
    );
    Ok(())
}
