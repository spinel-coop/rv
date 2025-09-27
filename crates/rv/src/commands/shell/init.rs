use super::Shell;
use crate::config::Config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("We don't yet support automatic ruby usage on this shell")]
    Unsupported,
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
                    "function _rv_autoload_hook --on-variable PWD --description 'Change Ruby version on directory change using rv'\n",
                    "    status --is-command-substitution; and return\n",
                    "    {} shell env fish | source\n",
                    "end\n"
                ),
                config.current_exe
            );
            Ok(())
        }
        Shell::Nu => {
            // TODO: Set up the `rv` autoload hook here.
            // It should change ruby version using `rv` whenever the user changes directory.
            // See their example for a change-of-directory hook:
            // <https://www.nushell.sh/book/hooks.html#automatically-activating-an-environment-when-entering-a-directory>
            Err(Error::Unsupported)
        }
    }
}
