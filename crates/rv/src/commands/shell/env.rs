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
                println!("set -gx {var} \"{}\"", fish_var_escape(val))
            }
            Ok(())
        }
        Shell::Nu => {
            // Emit JSON which will be run by `load-env`.
            // See <https://www.nushell.sh/commands/docs/load-env.html>
            let env_json = nu_env(unset, set);
            let serialized = serde_json::to_string(&env_json).expect("serializing JSON");
            println!("{}", serialized);
            Ok(())
        }
    }
}

fn nu_env(unset: Vec<&str>, set: Vec<(&str, String)>) -> serde_json::Value {
    // Map from environment variable names to their new values.
    // In nushell, empty JSON object means "unset this var."
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
    serde_json::Value::Object(env_changes)
}

// From uv's crates/uv-shell/src/lib.rs
// Assumes strings will be outputed as "str", so escapes any \ or " character
fn fish_var_escape(s: String) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nushell_env_serializes_changes() {
        let unset = vec!["RUBY_ROOT", "GEM_PATH"];
        let set = vec![
            ("PATH", "/tmp/bin".to_owned()),
            ("RUBY_ROOT", "/new/ruby".to_owned()),
        ];

        let env_json = nu_env(unset, set);

        let expected = json!({
            "RUBY_ROOT": "/new/ruby",
            "GEM_PATH": {},
            "PATH": "/tmp/bin",
        });

        assert_eq!(env_json, expected);
        let _ = serde_json::to_string(&env_json)
            .expect("Serializing the Nushell env changes to JSON should always succeed");
    }
}
