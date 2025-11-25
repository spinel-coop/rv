use std::{io, process::Command};

use rv_ruby::request::RubyRequest;

use crate::config::{self, Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("no matching ruby version found")]
    NoMatchingRuby,
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error(transparent)]
    ExecError(#[from] io::Error),
}

type Result<T> = miette::Result<T, Error>;

pub fn run(config: &Config, request: &RubyRequest, args: &[String]) -> Result<()> {
    let cmd = build_cmd(config, request, args)?;
    exec(cmd)
}

pub(crate) fn build_cmd(
    config: &Config,
    request: &RubyRequest,
    args: &[String],
) -> Result<Command> {
    let Some(ruby) = config.matching_ruby(request) else {
        return Err(Error::NoMatchingRuby);
    };
    let (unset, set) = config::env_for(Some(&ruby))?;
    let mut cmd = Command::new(ruby.executable_path());
    cmd.args(args);
    for var in unset {
        cmd.env_remove(var);
    }
    for (var, val) in set {
        cmd.env(var, val);
    }
    Ok(cmd)
}

#[cfg(unix)]
fn exec(mut cmd: Command) -> Result<()> {
    use std::os::unix::process::CommandExt;
    Err(cmd.exec().into())
}
