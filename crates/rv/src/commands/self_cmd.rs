use axoupdater::AxoUpdater;
use clap::{Args, Subcommand};

use crate::{GlobalArgs, update::is_homebrew_install};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    AxoupdateError(#[from] axoupdater::AxoupdateError),
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
    if is_homebrew_install() {
        println!(
            "Your copy of `rv` was installed via Homebrew. Run `brew upgrade rv` to update it."
        );
        return Ok(());
    }

    let mut updater = AxoUpdater::new_for("rv");

    if updater.load_receipt().is_err() || !updater.check_receipt_is_for_this_executable()? {
        println!(
            "Your copy of `rv` was not installed via a method that `rv self update` supports. Please update manually."
        );
        return Ok(());
    }

    if let Some(result) = updater.run().await? {
        println!("rv {} installed!", result.new_version);
    } else {
        println!("rv is already up to date!");
    }

    Ok(())
}

pub(crate) fn version() {
    println!("rv {}", env!("CARGO_PKG_VERSION"));
}
