use clap::{Args, Subcommand};

use crate::GlobalArgs;

pub mod new;

#[derive(Args)]
pub struct GemArgs {
    #[command(subcommand)]
    pub command: GemCommand,
}

#[derive(Subcommand)]
pub enum GemCommand {
    #[command(about = "Create a new gem scaffold")]
    New(new::NewArgs),
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    NewError(#[from] new::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) fn gem(global_args: &GlobalArgs, args: GemArgs) -> Result<()> {
    match args.command {
        GemCommand::New(new_args) => new::new(global_args, new_args)?,
    };

    Ok(())
}
