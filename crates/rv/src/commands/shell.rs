pub mod env;
pub mod init;

use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ShellArgs {
    #[command(subcommand)]
    pub command: ShellCommand,
}

#[derive(Subcommand)]
pub enum ShellCommand {
    #[command(about = "Configure your shell to use rv")]
    Init,
    #[command(hide = true)]
    Env,
}
