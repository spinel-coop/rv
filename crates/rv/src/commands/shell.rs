pub mod completions;
pub mod env;
pub mod init;

use crate::config::Config;
use clap::{Args, Subcommand};
use serde::Serialize;

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
#[group(required = true, multiple = false)]
pub struct ShellArgs {
    #[arg(value_enum)]
    pub shell: Option<Shell>,

    #[command(subcommand)]
    pub command: Option<ShellCommand>,
}

#[derive(Subcommand)]
pub enum ShellCommand {
    #[command(hide = true)]
    Init { shell: Shell },
    #[command(hide = true)]
    Completions { shell: Shell },
    #[command(hide = true)]
    Env { shell: Shell },
}

#[derive(clap::ValueEnum, Clone, Default, Debug, Serialize)]
pub enum Shell {
    #[default]
    Zsh,
    Bash,
    Fish,
    Nu,
}

impl std::fmt::Display for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zsh => write!(f, "zsh"),
            Self::Bash => write!(f, "bash"),
            Self::Fish => write!(f, "fish"),
            Self::Nu => write!(f, "nu"),
        }
    }
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = miette::Result<T, Error>;

pub fn setup(config: &Config, shell: Shell) -> Result<()> {
    use indoc::{formatdoc, printdoc};

    let name = shell.to_string();

    let header = formatdoc! {"
        Install rv's shell integration into {name} by running the commands below,
        or configuring your shell to do the equivalent."
    };

    let rv = &config.current_exe;

    match shell {
        Shell::Zsh => {
            printdoc! {"
                {header}

                echo 'eval \"$({rv} shell init zsh)\"' >> ~/.zshrc
                echo 'eval \"$({rv} shell completions zsh)\"' >> ~/.zshrc
            "};

            Ok(())
        }
        Shell::Bash => {
            printdoc! {"
                {header}

                echo 'eval \"$({rv} shell init bash)\"' >> ~/.bashrc
                echo 'eval \"$({rv} shell completions bash)\"' >> ~/.bashrc
            "};

            Ok(())
        }
        Shell::Fish => {
            printdoc! {"
                {header}

                echo '{rv} shell init fish | source' >> ~/.config/fish/config.fish
                echo '{rv} shell completions fish | source' >> ~/.config/fish/config.fish
            "};

            Ok(())
        }
        Shell::Nu => {
            printdoc! {"
                {header}

                echo 'mkdir ($nu.data-dir | path join \"vendor/autoload\")
                {rv} shell init nu | save -f ($nu.data-dir | path join \"vendor/autoload/rv.nu\")
                {rv} shell completions nu | save --append ($nu.data-dir | path join \"vendor/autoload/rv.nu\")' | save --append $nu.config-path
            "};

            Ok(())
        }
    }
}
