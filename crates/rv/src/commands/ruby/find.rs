use anstream::println;
use owo_colors::OwoColorize;
use rv_ruby::request::RubyRequest;

use crate::{GlobalArgs, config::Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("no matching ruby version found")]
    NoMatchingRuby,
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) fn find(global_args: &GlobalArgs, request: Option<RubyRequest>) -> Result<()> {
    let config = Config::new(global_args, request)?;

    let ruby = config.current_ruby().ok_or(Error::NoMatchingRuby)?;
    println!("{}", ruby.executable_path().cyan());
    Ok(())
}
