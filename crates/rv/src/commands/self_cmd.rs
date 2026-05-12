use clap::{Args, Subcommand};
use tracing::error;

use crate::update::{UpdateOutcome, run_update};
use crate::{GlobalArgs, update};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    UpdateError(#[from] update::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = miette::Result<T, Error>;

#[derive(Args)]
pub struct SelfArgs {
    #[command(subcommand)]
    pub command: SelfCommand,
}

#[derive(Subcommand)]
pub enum SelfCommand {
    #[command(about = "Update rv to the latest version")]
    Update,
    #[command(about = "Display rv's version")]
    Version,
}

pub(crate) async fn self_cmd(_global_args: &GlobalArgs, args: SelfArgs) -> Result<()> {
    match args.command {
        SelfCommand::Update => update().await?,
        SelfCommand::Version => version(),
    }

    Ok(())
}

pub(crate) async fn update() -> Result<()> {
    match run_update("install").await {
        Ok(UpdateOutcome::Installed(v)) => {
            eprintln!("✅ New version of `rv` {} installed!", v);
        }
        Ok(UpdateOutcome::UpdateAvailable(_latest)) => {}
        Ok(UpdateOutcome::AlreadyUpToDate) => {
            eprintln!("rv is already up to date!");
        }
        Err(e) => {
            error!("Self-update failed: {}", e);
        }
    }

    Ok(())
}

pub(crate) fn version() {
    println!("rv {}", env!("CARGO_PKG_VERSION"));
}
