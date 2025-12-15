use std::{
    io,
    path::Path,
    process::{Command, ExitStatus, Output},
};

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

#[derive(Debug, Default, Clone, Copy)]
pub(crate) enum CaptureOutput {
    #[default]
    No,
    /// Both stdout and stderr
    Both,
}

/// Shell out to the given ruby `version`, run it with the given arguments.
/// If given `version` is `None`, shell out to whatever version is pinned in a version
/// file, or to the default ruby version if no ruby version is found in version files.
/// By default, if the ruby isn't installed, install it (disabled via `no_install`).
/// The ruby's output may be captured, depending on `capture_output`. If you pass
/// `CaptureOutput::No`, this returns an empty `Output` struct.
pub(crate) async fn run<A: AsRef<std::ffi::OsStr>>(
    config: &Config,
    version: Option<RubyRequest>,
    no_install: bool,
    args: &[A],
    capture_output: CaptureOutput,
    cwd: Option<&Path>,
) -> Result<Output> {
    let request = match version {
        None => config.ruby_request()?,
        Some(version) => version,
    };
    if config.matching_ruby(&request).is_none() && !no_install {
        // Not installed, try to install it.
        // None means it'll install in whatever default ruby location it chooses.
        let install_dir = None;
        crate::commands::ruby::install::install(config, install_dir, &request, None).await?
    };
    let ruby = config
        .matching_ruby(&request)
        .ok_or(Error::NoMatchingRuby)?;
    let (unset, set) = config::env_for(Some(&ruby))?;
    let mut cmd = Command::new(ruby.executable_path());
    cmd.args(args);
    for var in unset {
        cmd.env_remove(var);
    }
    for (var, val) in set {
        cmd.env(var, val);
    }
    if let Some(path) = cwd {
        cmd.current_dir(path);
    }

    match capture_output {
        CaptureOutput::No => {
            exec(cmd).map(|()| Output {
                // Success
                status: ExitStatus::default(),
                // Both empty
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
        CaptureOutput::Both => Ok(cmd.output()?),
    }
}

#[cfg(unix)]
fn exec(mut cmd: Command) -> Result<()> {
    use std::os::unix::process::CommandExt;
    Err(cmd.exec().into())
}
