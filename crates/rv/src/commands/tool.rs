pub mod install;

use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ToolArgs {
    #[command(subcommand)]
    pub command: ToolCommand,
}

#[derive(Subcommand)]
pub enum ToolCommand {
    #[command(about = "Install a given gem as a tool")]
    Install {
        /// What to install
        gem: String,
    },
}
