use super::Shell;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = miette::Result<T, Error>;

pub fn init(shell: Shell) -> Result<()> {
    use indoc::printdoc;

    let current_exe = rv_dirs::current_exe()?;

    match shell {
        Shell::Zsh => {
            printdoc! {"
                autoload -U add-zsh-hook
                _rv_autoload_hook () {{
                    eval \"$({current_exe} shell env zsh)\"
                }}
                add-zsh-hook preexec _rv_autoload_hook
                _rv_autoload_hook
            "};
        }
        Shell::Bash => {
            printdoc! {"
                _rv_autoload_hook() {{
                    eval \"$({current_exe} shell env bash)\"
                }}
                if [[ \";${{PROMPT_COMMAND:-}};\" != *\";_rv_autoload_hook;\"* ]]
                then
                    PROMPT_COMMAND=\"_rv_autoload_hook${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}\"
                fi
                _rv_autoload_hook
            "};
        }
        Shell::Fish => {
            printdoc! {"
                function _rv_autoload_hook --on-event fish_preexec --description 'Change Ruby version before running every command'
                    {current_exe} shell env fish | source
                end
                _rv_autoload_hook
            "};
        }
        Shell::Nu => {
            printdoc! {"
                $env.config = ($env.config | upsert hooks.pre_execution {{
                    [
                        {{||
                            {current_exe} shell env nu | from json | load-env
                        }}
                    ]
                }})
            "};
        }
        Shell::PowerShell => {
            // PowerShell doesn't have a preexec hook, so we use the prompt function
            // which runs after each command (before displaying the next prompt).
            // This pattern matches Python's virtualenv activate.ps1.
            printdoc! {"
                if (Test-Path Function:\\__rv_original_prompt) {{
                    Remove-Item Function:\\__rv_original_prompt
                }}
                Copy-Item Function:\\prompt Function:\\__rv_original_prompt
                function global:prompt {{
                    Invoke-Expression (& '{current_exe}' shell env powershell)
                    __rv_original_prompt
                }}
                Invoke-Expression (& '{current_exe}' shell env powershell)
            "};
        }
    }

    Ok(())
}
