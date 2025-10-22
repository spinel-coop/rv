use crate::config::Config;
use std::io;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] rv_lockfile::ParseErrors),
    #[error(transparent)]
    Io(#[from] io::Error),
}

type Result<T> = std::result::Result<T, Error>;

pub async fn ci(_config: &Config, gemfile: camino::Utf8PathBuf) -> Result<()> {
    let gemfile_contents = std::fs::read_to_string(gemfile)?;
    let lockfile = rv_lockfile::parse(&gemfile_contents)?;
    Ok(())
}
