use std::path::PathBuf;

use camino::Utf8PathBuf;
use clap::Args;
use rv_ruby::request::RubyRequest;
use tracing::debug;

use crate::config::Config;
use crate::script_metadata;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("Script file not found: {0}")]
    ScriptNotFound(PathBuf),
    #[error("Script path is not valid UTF-8: {0}")]
    InvalidUtf8Path(PathBuf),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    RunError(#[from] crate::commands::ruby::run::Error),
}

type Result<T> = miette::Result<T, Error>;

#[derive(Args)]
pub struct RunArgs {
    /// Ruby version to use (overrides script metadata)
    #[arg(long)]
    pub ruby: Option<RubyRequest>,

    /// Don't install Ruby if missing
    #[arg(long)]
    pub no_install: bool,

    /// Script file to run
    pub script: PathBuf,

    /// Arguments to pass to the script
    #[arg(last = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

pub async fn run(config: &Config, args: RunArgs) -> Result<()> {
    if !args.script.exists() {
        return Err(Error::ScriptNotFound(args.script));
    }

    let script_path = args.script.canonicalize()?;
    let script_path_utf8: Utf8PathBuf = script_path
        .clone()
        .try_into()
        .map_err(|_| Error::InvalidUtf8Path(script_path.clone()))?;

    let ruby_version = if let Some(version) = args.ruby {
        debug!("Using Ruby version from --ruby flag: {}", version);
        Some(version)
    } else {
        let content = std::fs::read_to_string(&script_path)?;
        if let Some(metadata) = script_metadata::parse(&content) {
            if let Some(ref version) = metadata.requires_ruby {
                debug!("Using Ruby version from script metadata: {}", version);
            }
            metadata.requires_ruby
        } else {
            debug!("No script metadata found, falling back to config file detection");
            None
        }
    };

    let mut ruby_args: Vec<String> = vec![script_path_utf8.to_string()];
    ruby_args.extend(args.args);

    crate::commands::ruby::run::run(
        config,
        ruby_version,
        args.no_install,
        &ruby_args,
        Default::default(),
        Default::default(),
    )
    .await
    .map(|_| ())?;

    Ok(())
}
