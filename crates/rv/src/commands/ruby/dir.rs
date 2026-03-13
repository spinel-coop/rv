use anstream::println;
use owo_colors::OwoColorize;

use crate::{GlobalArgs, config::Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) fn dir(global_args: &GlobalArgs) -> Result<()> {
    let config = Config::new(global_args, None)?;

    let Some(ruby_dir) = config.ruby_dirs.first() else {
        tracing::error!("No Ruby directories found");
        return Ok(());
    };

    println!("{}", ruby_dir.as_str().cyan());

    Ok(())
}
