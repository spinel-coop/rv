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
        /// What gem server to use.
        #[arg(long, default_value = "https://gem.coop/")]
        gem_server: String,
    },
}
