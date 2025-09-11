pub mod completions;
pub mod env;
pub mod init;

use clap::{Args, Subcommand};
use serde::Serialize;

#[derive(Args)]
pub struct ShellArgs {
    #[command(subcommand)]
    pub command: ShellCommand,
}

#[derive(Subcommand)]
pub enum ShellCommand {
    #[command(about = "Configure your shell to use rv")]
    Init {
        /// The shell to initialize (zsh, bash and fish so far)
        shell: Shell,
    },
    #[command()]
    Completions {
        /// The shell to print completions for (zsh, bash and fish so far)
        shell: Shell,
    },
    #[command(hide = true)]
    Env {
        /// The shell to configure (zsh, bash and fish so far)
        shell: Shell,
    },
}

#[derive(clap::ValueEnum, Clone, Default, Debug, Serialize)]
pub enum Shell {
    #[default]
    Zsh,
    Bash,
    Fish,
}
