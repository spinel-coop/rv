use rv_gem_types::requirement::Requirement;
use rv_ruby::version::RubyVersion;

use crate::{
    commands::tool::install::{Error, Result},
    config::Config,
};

/// Which ruby version should be used to run a tool?
/// This takes the tool's Ruby version constraints as a parameter.
pub async fn ruby_to_use_for(
    config: &Config<'_>,
    ruby_constraints: &Requirement,
) -> Result<RubyVersion> {
    let installed_rubies = config.rubies();

    match ruby_constraints.find_match_in(&installed_rubies, false) {
        Some(local_ruby) => Ok(local_ruby.version),
        None => {
            let remote_rubies = &config.remote_rubies().await;

            match ruby_constraints
                .find_match_in(remote_rubies, false)
                .or_else(|| ruby_constraints.find_match_in(remote_rubies, true))
            {
                Some(remote_ruby) => Ok(remote_ruby.version),
                None => Err(Error::NoMatchingRuby {
                    requirement: ruby_constraints.clone(),
                }),
            }
        }
    }
}
