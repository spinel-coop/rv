use std::io::stdout;

use clap_complete::{Shell as ClapCompleteShell, generate};

use super::Shell;

pub fn shell_completions(cmd: &mut clap::Command, shell: Shell) {
    let name = cmd.get_name().to_owned();
    match shell {
        Shell::Zsh => {
            let clap_complete_shell: ClapCompleteShell = ClapCompleteShell::Zsh;
            generate(clap_complete_shell, cmd, name, &mut stdout());
        }
        Shell::Bash => {
            let clap_complete_shell: ClapCompleteShell = ClapCompleteShell::Bash;
            generate(clap_complete_shell, cmd, name, &mut stdout());
        }
        Shell::Fish => {
            let clap_complete_shell: ClapCompleteShell = ClapCompleteShell::Fish;
            generate(clap_complete_shell, cmd, name, &mut stdout());
        }
        Shell::Nu => {
            let clap_complete_shell = clap_complete_nushell::Nushell;
            generate(clap_complete_shell, cmd, name, &mut stdout());
        }
        Shell::PowerShell => {
            let clap_complete_shell: ClapCompleteShell = ClapCompleteShell::PowerShell;
            generate(clap_complete_shell, cmd, name, &mut stdout());
        }
    }
}
