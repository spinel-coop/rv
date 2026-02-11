use camino::Utf8PathBuf;
use clap::Args;
use fs_err as fs;
use rv_ruby::request::RubyRequest;
use tracing::debug;

use crate::GlobalArgs;
use crate::commands::ruby::run::{Invocation, Program};
use crate::script_metadata;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("Could not read file {file}: {e}")]
    CouldNotRead { file: String, e: std::io::Error },
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    RunError(#[from] crate::commands::ruby::run::Error),
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
    args: Vec<String>,
}

pub(crate) async fn run(global_args: &GlobalArgs, args: RunArgs) -> Result<()> {
    let (script, cmd_args) = args.args.split_first().unwrap();
    let script = Utf8PathBuf::from(script);
    let mut cmd_args = Vec::from(cmd_args);
    let mut ruby_version = None;

    let script_filepath = rv_dirs::canonicalize_utf8(&script).ok();
    let invocation = if script_filepath
        .map(|path| path.exists())
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

    crate::commands::ruby::run::run(
        invocation,
        global_args,
        ruby_version,
        args.no_install,
        &cmd_args,
        Default::default(),
        Default::default(),
    )
    .await?;

    Ok(())
}
