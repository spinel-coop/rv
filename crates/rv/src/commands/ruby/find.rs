use anstream::println;
use owo_colors::OwoColorize;
use rv_ruby::request::RubyRequest;

use crate::{GlobalArgs, config::Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("{}", .requested.no_installed_match())]
    NoMatchingRuby {
        requested: crate::config::RequestedRuby,
    },
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) fn find(global_args: &GlobalArgs, request: Option<RubyRequest>) -> Result<()> {
    let config = Config::new(global_args, request)?;

    let ruby = config.current_ruby().ok_or_else(|| Error::NoMatchingRuby {
        requested: config.requested_ruby.clone(),
    })?;
    println!("{}", ruby.executable_path().cyan());
    Ok(())
}
