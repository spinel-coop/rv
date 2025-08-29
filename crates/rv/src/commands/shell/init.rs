use super::Shell;
use crate::config::Config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = miette::Result<T, Error>;

pub fn init(config: &Config, shell: Shell) -> Result<()> {
    match shell {
        Shell::Zsh => {
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
        Shell::Bash => {
            print!(
                concat!(
                    "_rv_autoload_hook() {{\n",
                    "    eval \"$({} shell env bash)\"\n",
                    "}}\n",
                    "_rv_autoload_hook\n",
                    "_chpwd_hook() {{\n",
                    "    if [[ \"$PWD\" != \"$_OLDPWD\" ]]; then\n",
                    "        _rv_autoload_hook\n",
                    "        _OLDPWD=\"$PWD\"\n",
                    "    fi\n",
                    "}}\n",
                    "_OLDPWD=\"$PWD\"\n",
                    "PROMPT_COMMAND=\"_chpwd_hook${{PROMPT_COMMAND:+; $PROMPT_COMMAND}}\"\n",
                ),
                config.current_exe
            );
            Ok(())
        }
        Shell::Fish => {
            print!(
                concat!(
                    "function _rv_autoload_hook --on-variable PWD\n",
                    "    \"{}\" shell env fish | source\n",
                    "end\n",
                    "_rv_autoload_hook\n"
                ),
                config.current_exe
            );
            Ok(())
        }
    }
}
