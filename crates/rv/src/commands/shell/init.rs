use crate::config::Config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Sorry, rv doesn't support the {:?} shell, yet", shell)]
    UnsupportedShell { shell: super::Shell },
}

type Result<T> = miette::Result<T, Error>;

pub fn init(config: &Config, shell: super::Shell) -> Result<()> {
    match shell {
        super::Shell::Zsh => {
            print!(
                concat!(
                    "autoload -U add-zsh-hook\n",
                    "_rv_autoload_hook () {{\n",
                    "    eval \"$({} shell env zsh)\"\n",
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
