use std::path::PathBuf;

use camino::Utf8PathBuf;
use clap::Args;
use rv_ruby::request::RubyRequest;
use tracing::debug;

use crate::commands::ruby::run::{Invocation, Program};
use crate::config::Config;
use crate::script_metadata;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
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

pub async fn run(config: &Config, args: RunArgs) -> Result<()> {
    let (script, cmd_args) = args.args.split_first().unwrap();
    let script = Utf8PathBuf::from(script);
    let mut cmd_args = Vec::from(cmd_args);

    let mut ruby_version = if let Some(version) = args.ruby {
        debug!("Using Ruby version from --ruby flag: {}", version);
        Some(version)
    } else {
        None
    };

    let invocation = if script.canonicalize_utf8()?.exists() {
        let content = std::fs::read_to_string(&script)?;
        if let Some(metadata) = script_metadata::parse(&content) {
            if let Some(ref version) = metadata.requires_ruby {
                debug!("Using Ruby version from script metadata: {}", version);
            }
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

    crate::commands::ruby::run::run(
        invocation,
        config,
        ruby_version,
        args.no_install,
        &cmd_args,
        Default::default(),
        Default::default(),
    )
    .await?;

    Ok(())
}
