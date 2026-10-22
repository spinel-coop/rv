use super::Shell;
use super::powershell_escape;
use crate::{GlobalArgs, config::Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error("Could not serialize JSON: {0}")]
    Serde(#[from] serde_json::Error),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) fn env(global_args: &GlobalArgs, shell: Shell) -> Result<()> {
    let config = Config::new(global_args, None)?;
    let ruby = config.best_ruby();
    let (unset, set) = config.env_for(ruby.as_ref())?.split();

    match shell {
        Shell::Zsh | Shell::Bash => {
            if !unset.is_empty() {
                println!("unset {}", unset.join(" "));
            }

            for (var, val) in set {
                // On Windows, normalize paths for Unix shells:
                // - Replace `\` path separators with `/` (bash/zsh always use forward slashes)
                // - Replace `;` path list separators with `:` (std::env::join_paths
                //   uses `;` on Windows, but bash/zsh always use `:`)
                // This also prevents shell_escape from adding unnecessary quotes
                // around values that only contain backslashes as "special" characters.
                #[cfg(windows)]
                let val = val.replace('\\', "/").replace(';', ":");

                println!("export {var}={}", shell_escape::unix::escape(val.into()))
            }

            println!("hash -r");
            Ok(())
        }
        Shell::Fish => {
            if !unset.is_empty() {
                println!("set -ge {}", unset.join(" "))
            }
            for (var, val) in set {
                // Same Windows path normalization as bash/zsh — fish uses `/` and `:`
                #[cfg(windows)]
                let val = val.replace('\\', "/").replace(';', ":");

                println!("set -gx {var} \"{}\"", fish_var_escape(val))
            }
            Ok(())
        }
        Shell::Nu => {
            // Emit JSON which will be run by `load-env`.
            // See <https://www.nushell.sh/commands/docs/load-env.html>
            let env_json = nu_env(unset, set);
            let serialized = serde_json::to_string(&env_json)?;
            println!("{}", serialized);
            Ok(())
        }
        Shell::PowerShell => {
            // PowerShell uses $env:VAR for environment variables
            // Use backticks to escape special characters (following uv's pattern)
            for var in unset {
                println!("Remove-Item Env:\\{var} -ErrorAction SilentlyContinue");
            }
            for (var, val) in set {
                println!("$env:{var} = \"{}\"", powershell_escape(&val));
            }
            Ok(())
        }
    }
}

fn nu_env(unset: Vec<&str>, set: Vec<(&str, String)>) -> serde_json::Value {
    // Map from environment variable names to their new values.
    // `null` means "unset this var": `load-env` can't unset one, so the hook
    // hides these with `hide-env`. Nushell keeps PATH as a list, so it's
    // sent as one.
    let mut env_changes = serde_json::Map::with_capacity(set.len() + unset.len());
    for var in unset {
        env_changes.insert(var.to_owned(), serde_json::Value::Null);
    }
    for (var, val) in set {
        let val = match var {
            // An empty PATH has no entries, rather than one empty one.
            "PATH" if val.is_empty() => serde_json::Value::Array(Vec::new()),
            "PATH" => std::env::split_paths(&val)
                .map(|p| serde_json::Value::String(p.to_string_lossy().into_owned()))
                .collect(),
            _ => serde_json::Value::String(val),
        };
        env_changes.insert(var.to_owned(), val);
    }
    serde_json::Value::Object(env_changes)
}

// Credit to uv's crates/uv-shell/src/lib.rs
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
        let path = std::env::join_paths(["/tmp/bin", "/usr/bin"]).unwrap();
        let set = vec![
            ("PATH", path.into_string().unwrap()),
            ("RUBY_ROOT", "/new/ruby".to_owned()),
        ];

        let env_json = nu_env(unset, set);

        let expected = json!({
            "RUBY_ROOT": "/new/ruby",
            "GEM_PATH": null,
            "PATH": ["/tmp/bin", "/usr/bin"],
        });

        assert_eq!(env_json, expected);
        let _ = serde_json::to_string(&env_json)
            .expect("Serializing the Nushell env changes to JSON should always succeed");
    }

    #[test]
    fn fish_var_escape_handles_special_chars() {
        // Typical Unix path passes through unchanged
        assert_eq!(
            fish_var_escape("/home/user/.rubies/ruby-3.4.1/bin".to_owned()),
            "/home/user/.rubies/ruby-3.4.1/bin"
        );

        // Backslashes in paths are escaped (rare on Unix, but possible)
        assert_eq!(
            fish_var_escape("/path/with\\backslash".to_owned()),
            "/path/with\\\\backslash"
        );

        // Double quotes in directory names are escaped
        assert_eq!(
            fish_var_escape("/home/user/\"My Projects\"/bin".to_owned()),
            "/home/user/\\\"My Projects\\\"/bin"
        );
    }

    #[test]
    fn powershell_escape_handles_special_chars() {
        // Typical Windows path passes through unchanged
        assert_eq!(
            powershell_escape("C:\\Users\\eric\\.rubies\\ruby-3.4.1\\bin"),
            "C:\\Users\\eric\\.rubies\\ruby-3.4.1\\bin"
        );

        // Dollar signs are escaped (prevents PowerShell variable expansion)
        assert_eq!(powershell_escape("$HOME/.rubies"), "`$HOME/.rubies");

        // Double quotes in paths are escaped
        assert_eq!(
            powershell_escape("C:\\Users\\eric\\\"My Projects\""),
            "C:\\Users\\eric\\`\"My Projects`\""
        );

        // Backticks are escaped (PowerShell escape character)
        assert_eq!(powershell_escape("path`with`ticks"), "path``with``ticks");
    }
}
