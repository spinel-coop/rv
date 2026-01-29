use anstream::println;
use owo_colors::OwoColorize;
use rv_ruby::request::RubyRequest;

use crate::config::Config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    InvalidVersion(#[from] rv_ruby::request::RequestError),
    #[error("no matching ruby version found")]
    NoMatchingRuby,
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
}

type Result<T> = miette::Result<T, Error>;

pub fn find(config: &Config, version: Option<RubyRequest>) -> Result<()> {
    let request = match version {
        None => config.ruby_request(),
        Some(version) => version,
    };
    if let Some(ruby) = config.matching_ruby(&request) {
        println!("{}", ruby.executable_path().cyan());
        Ok(())
    } else {
        Err(Error::NoMatchingRuby)
    }
}
