use std::io::stdout;

use clap_complete::{Shell as ClapCompleteShell, generate};

use super::Shell;

impl From<Shell> for ClapCompleteShell {
    fn from(shell: Shell) -> Self {
        match shell {
            Shell::Zsh => ClapCompleteShell::Zsh,
            Shell::Bash => ClapCompleteShell::Bash,
            Shell::Fish => ClapCompleteShell::Fish,
        }
    }
}

pub fn shell_completions(cmd: &mut clap::Command, shell: Shell) {
    let clap_complete_shell: ClapCompleteShell = shell.into();
    let name = cmd.get_name().to_owned();
    generate(clap_complete_shell, cmd, name, &mut stdout());
}
