use std::{
    io,
    path::PathBuf,
    process::{Command, ExitStatus, Output},
};

use camino::{Utf8Path, Utf8PathBuf};
use rv_ruby::request::RubyRequest;
use tracing::debug;

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

pub(crate) enum Program {
    Ruby,
    Tool {
        executable_path: Utf8PathBuf,
        extra_paths: Vec<PathBuf>,
    },
}

pub(crate) struct Invocation {
    pub program: Program,

    pub env: Vec<(&'static str, String)>,
}

impl Invocation {
    pub fn ruby(env: Vec<(&'static str, String)>) -> Self {
        Self {
            program: Program::Ruby,
            env,
        }
    }
}

/// Shell out to the given ruby `version`, run it with the given arguments.
/// If given `version` is `None`, shell out to whatever version is pinned in a version
/// file, or to the default ruby version if no ruby version is found in version files.
/// By default, if the ruby isn't installed, install it (disabled via `no_install`).
/// The ruby's output may be captured, depending on `capture_output`. If you pass
/// `CaptureOutput::No`, this returns an empty `Output` struct.
pub(crate) async fn run<A: AsRef<std::ffi::OsStr>>(
    invocation: Invocation,
    config: &Config,
    version: Option<RubyRequest>,
    no_install: bool,
    args: &[A],
    capture_output: CaptureOutput,
    cwd: Option<&Utf8Path>,
) -> Result<Output> {
    let request = match version {
        None => config.ruby_request(),
        Some(version) => version,
    };
    let install = !no_install;
    if config.matching_ruby(&request).is_none() && install {
        // Not installed, try to install it.
        // None means it'll install in whatever default ruby location it chooses.
        debug!("Ruby not found, so installing {request}");
        let install_dir = None;
        let tarball_path = None;
        crate::commands::ruby::install::install(
            config,
            install_dir,
            Some(request.clone()),
            tarball_path,
        )
        .await?
    };
    run_no_install(invocation, config, &request, args, capture_output, cwd)
}

/// Run, without installing the Ruby version if necessary.
pub(crate) fn run_no_install<A: AsRef<std::ffi::OsStr>>(
    invocation: Invocation,
    config: &Config,
    request: &RubyRequest,
    args: &[A],
    capture_output: CaptureOutput,
    cwd: Option<&Utf8Path>,
) -> Result<Output> {
    let ruby = config.matching_ruby(request).ok_or(Error::NoMatchingRuby)?;
    let ((unset, set), executable_path) = match invocation.program {
        Program::Ruby => (config::env_for(Some(&ruby))?, ruby.executable_path()),
        Program::Tool {
            executable_path,
            extra_paths,
        } => (
            config::env_with_path_for(Some(&ruby), extra_paths)?,
            executable_path,
        ),
    };
    let mut cmd = Command::new(executable_path);
    cmd.args(args);
    for var in unset {
        cmd.env_remove(var);
    }
    for (key, val) in set {
        cmd.env(key, val);
    }
    for (key, val) in invocation.env {
        cmd.env(key, val);
    }
    if let Some(path) = cwd {
        cmd.current_dir(path);
    }

    debug!("Running command: {:?}", cmd);
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
