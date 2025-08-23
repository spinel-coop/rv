use super::Shell;
use crate::config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    ConfigError(#[from] config::Error),
    #[error("No Ruby installations found in configuration.")]
    NoRubyFound,
}

type Result<T> = miette::Result<T, Error>;

pub fn env(config: &config::Config, shell: Shell) -> Result<()> {
    let ruby = config.project_ruby();
    let (unset, set) = config::env_for(ruby.as_ref())?;

    match shell {
        Shell::Zsh => {
            if !unset.is_empty() {
                println!("unset {}", unset.join(" "));
            }

            for (var, val) in set {
                println!("export {var}={}", shell_escape::escape(val.into()))
            }

            println!("hash -r");
            Ok(())
        }
    }
}
