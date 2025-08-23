pub mod env;
pub mod init;

use std::str::FromStr;

use clap::{Args, Subcommand};

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

#[derive(Debug, Clone)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
    Nushell,
    Powershell,
    Unknown { name: String },
}

impl FromStr for Shell {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, std::convert::Infallible> {
        let shell = match s {
            "zsh" => Shell::Zsh,
            "bash" => Shell::Bash,
            "fish" => Shell::Fish,
            "nushell" => Shell::Nushell,
            "powershell" => Shell::Powershell,
            _ => Shell::Unknown {
                name: s.to_string(),
            },
        };
        Ok(shell)
    }
}

impl From<&str> for Shell {
    fn from(val: &str) -> Self {
        Shell::from_str(val).unwrap()
    }
}
