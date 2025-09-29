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
        Shell::Zsh | Shell::Bash => {
            if !unset.is_empty() {
                println!("unset {}", unset.join(" "));
            }

            for (var, val) in set {
                println!("export {var}={}", shell_escape::escape(val.into()))
            }

            println!("hash -r");
            Ok(())
        }
        Shell::Fish => {
            if !unset.is_empty() {
                println!("set -ge {}", unset.join(" "))
            }
            for (var, val) in set {
                println!("set -gx {var} \"{}\"", backslack_escape(val))
            }
            Ok(())
        }
        Shell::Nu => {
            // Emit JSON which will be run by `load-env`.
            // See <https://www.nushell.sh/commands/docs/load-env.html>
            // Map from environment variable names to their new values.
            let mut env_changes = serde_json::Map::with_capacity(set.len() + unset.len());
            for var in unset {
                env_changes.insert(
                    var.to_owned(),
                    serde_json::Value::Object(Default::default()),
                );
            }
            for (var, val) in set {
                env_changes.insert(var.to_owned(), serde_json::Value::String(val));
            }
            let serialized = serde_json::to_string(&env_changes).expect("serializing JSON");
            println!("{}", serialized);
            Ok(())
        }
    }
}

// From uv's crates/uv-shell/src/lib.rs
// Assumes strings will be outputed as "str", so escapes any \ or " character
fn backslack_escape(s: String) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '"' => escaped.push('\\'),
            _ => {}
        }
        escaped.push(c)
    }
    escaped
}
