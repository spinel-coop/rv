use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use fs_err as fs;
use rv_ruby::request::RubyRequest;
use std::path::PathBuf;
use std::process::{Command, Output};
use tracing::debug;

use crate::script_metadata;
use crate::{GlobalArgs, config::Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("Could not read file {file}: {e}")]
    CouldNotRead { file: String, e: std::io::Error },
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("no matching ruby version found")]
    NoMatchingRuby,
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error(transparent)]
    InstallError(#[from] crate::commands::ruby::install::Error),
}

type Result<T> = miette::Result<T, Error>;

#[derive(Args)]
pub struct RunArgs {
    /// Ruby version to use.
    #[arg(long)]
    pub ruby: Option<RubyRequest>,

    /// By default, rv will install Ruby if needed.
    /// If this flag is given, rv will exit with an error instead of installing.
    #[arg(long)]
    pub no_install: bool,

    /// What to run with Ruby available, e.g. `ruby myscript.rb`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true, value_names = ["COMMAND", "ARGS"])]
    pub args: Vec<String>,
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

pub(crate) async fn run(global_args: &GlobalArgs, args: RunArgs) -> Result<()> {
    let (script, cmd_args) = args.args.split_first().unwrap();
    let script = Utf8PathBuf::from(script);
    let mut cmd_args = Vec::from(cmd_args);
    let mut ruby_version = None;

    let script_filepath = rv_dirs::canonicalize_utf8(&script).ok();
    let invocation = if script_filepath
        .map(|path| path.is_file())
        .unwrap_or_default()
    {
        let content = fs::read_to_string(&script)?;
        if let Some(metadata) = script_metadata::parse(&content)
            && let Some(ref version) = metadata.requires_ruby
        {
            debug!("Using Ruby version from script metadata: {}", version);
            ruby_version = metadata.requires_ruby
        }

        cmd_args.insert(0, script.into());
        Invocation::ruby(vec![])
    } else {
        Invocation {
            program: Program::Tool {
                executable_path: script,
                extra_paths: vec![],
            },
            env: vec![],
        }
    };

    if let Some(version) = args.ruby {
        debug!("Using Ruby version from --ruby flag: {}", version);
        ruby_version = Some(version)
    };

    run_command(
        invocation,
        global_args,
        ruby_version,
        args.no_install,
        cmd_args,
    )
    .await
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

pub(crate) async fn run_command(
    invocation: Invocation,
    global_args: &GlobalArgs,
    request: Option<RubyRequest>,
    no_install: bool,
    args: Vec<String>,
) -> Result<()> {
    let config = &Config::with_settings(global_args, request)?;

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

    let cmd = prepare_command(invocation, config, args, Default::default())?;

    debug!("Running command: {:?}", cmd);
    exec(cmd)
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
