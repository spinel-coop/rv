use std::str::FromStr;

use crate::{commands, config::Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Sorry, rv doesn't support the {:?} shell, yet", shell)]
    UnsupportedShell { shell: Shell },
}

type Result<T> = miette::Result<T, Error>;

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

    fn from_str(
        s: &str,
    ) -> std::result::Result<commands::shell::init::Shell, std::convert::Infallible> {
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
        Shell::from_str(val).unwrap_or_else(|_| Shell::Unknown {
            name: val.to_string(),
        })
    }
}

pub fn init(config: &Config, shell: Shell) -> Result<()> {
    match shell {
        Shell::Zsh => {
            print!(
                concat!(
                    "autoload -U add-zsh-hook\n",
                    "_rv_autoload_hook () {{\n",
                    "    eval \"$({} shell env)\"\n",
                    "}}\n",
                    "add-zsh-hook chpwd _rv_autoload_hook\n",
                    "_rv_autoload_hook\n",
                ),
                config.current_exe
            );
            Ok(())
        }
        _ => Err(Error::UnsupportedShell { shell }),
    }
}
