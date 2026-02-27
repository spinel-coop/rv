use std::{
    io,
    path::PathBuf,
    process::{Command, Output},
};

use camino::{Utf8Path, Utf8PathBuf};
use rv_ruby::request::RubyRequest;
use tracing::debug;

use crate::{GlobalArgs, config::Config};

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

    pub fn tool(executable: &str, env: Vec<(&'static str, String)>) -> Self {
        Self {
            program: Program::Tool {
                executable_path: executable.into(),
                extra_paths: vec![],
            },
            env,
        }
    }
}

/// Shell out to the given ruby `request`, run it with the given arguments.
/// If given `request` is `None`, shell out to whatever version is pinned in a version
/// file, or to the default ruby version if no ruby version is found in version files.
/// By default, if the ruby isn't installed, install it (disabled via `no_install`).
pub(crate) async fn run(
    invocation: Invocation,
    global_args: &GlobalArgs,
    request: Option<RubyRequest>,
    no_install: bool,
    args: Vec<String>,
) -> Result<()> {
    let config = &Config::new(global_args, request)?;

    let install = !no_install;
    if config.current_ruby().is_none() && install {
        let request = config.ruby_request();

        // Not installed, try to install it.
        // None means it'll install in whatever default ruby location it chooses.
        debug!("Ruby not found, so installing {request}");
        let install_dir = None;
        let tarball_path = None;
        crate::commands::ruby::install::install(
            global_args,
            install_dir,
            Some(request),
            tarball_path,
            false,
        )
        .await?
    };

    let cmd = prepare_command(invocation, config, args, None)?;

    debug!("Running command: {:?}", cmd);
    exec(cmd)
}

/// Run, without installing the Ruby version if necessary, and capturing output.
pub(crate) fn capture_run_no_install(
    invocation: Invocation,
    config: &Config,
    args: Vec<String>,
    cwd: Option<&Utf8Path>,
) -> Result<Output> {
    let mut cmd = prepare_command(invocation, config, args, cwd)?;

    debug!("Running command: {:?}, and capturing output", cmd);

    Ok(cmd.output()?)
}

fn prepare_command(
    invocation: Invocation,
    config: &Config,
    args: Vec<String>,
    cwd: Option<&Utf8Path>,
) -> Result<Command> {
    let ruby = config.current_ruby().ok_or(Error::NoMatchingRuby)?;
    let ((unset, set), executable_path) = match invocation.program {
        Program::Ruby => (config.env_for(Some(&ruby))?.split(), ruby.executable_path()),
        Program::Tool {
            executable_path,
            extra_paths,
        } => {
            let (unset, set) = config.env_with_path_for(Some(&ruby), extra_paths)?.split();

            // On Windows, Rust's Command doesn't consult PATHEXT to resolve
            // .cmd/.bat files (rust-lang/rust#94743). Ruby tools like irb, gem,
            // and rake are .cmd batch files on Windows, so we resolve the full
            // path ourselves — following the pattern used by uv's WindowsRunnable.
            #[cfg(windows)]
            let executable_path = resolve_tool_on_windows(&executable_path, &set);

            ((unset, set), executable_path)
        }
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

    Ok(cmd)
}

/// On Windows, resolve a tool name to its full path by searching PATH directories
/// for files with standard executable extensions (.exe, .cmd, .bat).
///
/// Rust's `Command` doesn't consult PATHEXT (rust-lang/rust#94743), so
/// `Command::new("irb")` can't find `irb.cmd`. We search the PATH we've built
/// (which includes Ruby's bin/ directory) to find the actual file. Once resolved,
/// `Command::new("path/to/irb.cmd")` works because Rust 1.77.2+ handles .cmd
/// dispatch via CreateProcessW — the same mechanism rv already uses for ruby.cmd.
#[cfg(windows)]
fn resolve_tool_on_windows(executable: &Utf8Path, env_vars: &[(&str, String)]) -> Utf8PathBuf {
    // If the path already has an extension, return as-is.
    if executable.extension().is_some() {
        return executable.to_owned();
    }

    // Get PATH from the environment we're about to set on the command.
    let path_value = env_vars
        .iter()
        .find(|(k, _)| *k == "PATH")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");

    // Search PATH directories for the executable with standard Windows extensions.
    let extensions = ["exe", "cmd", "bat"];
    for dir in std::env::split_paths(path_value) {
        for ext in &extensions {
            let candidate = dir.join(format!("{}.{}", executable, ext));
            if candidate.exists()
                && let Ok(utf8) = Utf8PathBuf::try_from(candidate)
            {
                debug!("Resolved tool {executable} to {utf8}");
                return utf8;
            }
        }
    }

    debug!("Could not resolve tool {executable}, using as-is");
    executable.to_owned()
}

/// Spawns a command exec style.
/// On Unix, replaces the current process with the child.
/// On Windows, spawns the child, waits, and exits with the same code.
#[cfg(unix)]
fn exec(mut cmd: Command) -> Result<()> {
    use std::os::unix::process::CommandExt;
    Err(cmd.exec().into())
}

#[cfg(windows)]
fn exec(mut cmd: Command) -> Result<()> {
    use std::process::Stdio;

    cmd.stdin(Stdio::inherit());
    let status = cmd.status()?;

    #[allow(clippy::exit)]
    std::process::exit(status.code().unwrap_or(1))
}
