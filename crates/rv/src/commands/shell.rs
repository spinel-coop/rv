pub mod env;
pub mod init;

use clap::{Args, Subcommand};

use init::Shell;

#[derive(Args)]
pub struct ShellArgs {
    #[command(subcommand)]
    pub command: ShellCommand,
}

#[derive(Subcommand)]
pub enum ShellCommand {
    #[command(about = "Configure your shell to use rv")]
    Init {
        /// The shell to initialize
        shell: Shell,
    },
    #[command(hide = true)]
    Env,
}
