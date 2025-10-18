use anstream::println;
use owo_colors::OwoColorize;
use rv_ruby::request::RubyRequest;

use crate::config::Config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("no matching ruby version found")]
    NoMatchingRuby,
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
}

type Result<T> = miette::Result<T, Error>;

/// Uninstall the version ruby
pub async fn uninstall(config: &Config, request: RubyRequest) -> Result<()> {
    if let Some(ruby) = config.matching_ruby(&request) {
        let ruby_path = ruby.path;
        println!("{}", ruby_path.cyan());

        //delete install ruby path
        fs_err::remove_dir_all(ruby_path)
            .unwrap_or_else(|_| panic!("remove the ruby {} version is error", request));
        Ok(())
    } else {
        Err(Error::NoMatchingRuby)
    }
}
