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
    #[error(transparent)]
    InstallError(#[from] crate::commands::ruby::install::Error),
}

type Result<T> = miette::Result<T, Error>;

pub async fn run(
    config: &Config,
    request: &RubyRequest,
    no_install: bool,
    args: &[String],
) -> Result<()> {
    if config.matching_ruby(request).is_none() && !no_install {
        // Not installed, try to install it.
        // None means it'll install in whatever default ruby location it chooses.
        let install_dir = None;
        crate::commands::ruby::install::install(config, install_dir, request, None).await?
    };
    let ruby = config.matching_ruby(request).ok_or(Error::NoMatchingRuby)?;
    let (unset, set) = config::env_for(Some(&ruby))?;
    let mut cmd = Command::new(ruby.executable_path());
    cmd.args(args);
    for var in unset {
        cmd.env_remove(var);
    }
    for (var, val) in set {
        cmd.env(var, val);
    }

    exec(cmd)
}

#[cfg(unix)]
fn exec(mut cmd: Command) -> Result<()> {
    use std::os::unix::process::CommandExt;
    Err(cmd.exec().into())
}
