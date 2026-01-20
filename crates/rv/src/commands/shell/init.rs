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
                    "add-zsh-hook preexec _rv_autoload_hook\n",
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
                    "trap '[[ \"$BASH_COMMAND\" != \"$PROMPT_COMMAND\" ]] && _rv_autoload_hook' DEBUG\n",
                    "_rv_autoload_hook\n",
                ),
                config.current_exe
            );
            Ok(())
        }
        Shell::Fish => {
            print!(
                concat!(
                    "function _rv_autoload_hook --on-event fish_preexec --description 'Change Ruby version before running every command'\n",
                    "    {} shell env fish | source\n",
                    "end\n",
                    "_rv_autoload_hook\n"
                ),
                config.current_exe
            );
            Ok(())
        }
        Shell::Nu => {
            print!(
                concat!(
                    "$env.config = ($env.config | upsert hooks.pre_execution {{\n",
                    "    [\n",
                    "        {{||\n",
                    "            {} shell env nu | from json | load-env\n",
                    "        }}\n",
                    "    ]\n",
                    "}})\n",
                ),
                config.current_exe
            );
            Ok(())
        }
        Shell::PowerShell => {
            // PowerShell doesn't have a preexec hook, so we use the prompt function
            // which runs after each command (before displaying the next prompt).
            // This pattern matches Python's virtualenv activate.ps1.
            print!(
                concat!(
                    "if (Test-Path Function:\\__rv_original_prompt) {{\n",
                    "    Remove-Item Function:\\__rv_original_prompt\n",
                    "}}\n",
                    "Copy-Item Function:\\prompt Function:\\__rv_original_prompt\n",
                    "function global:prompt {{\n",
                    "    Invoke-Expression (& '{}' shell env powershell)\n",
                    "    __rv_original_prompt\n",
                    "}}\n",
                    "Invoke-Expression (& '{}' shell env powershell)\n",
                ),
                config.current_exe, config.current_exe
            );
            Ok(())
        }
    }
}
