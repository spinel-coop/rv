//! Checks on the environment rv inherited from the shell.
//!
//! These compare the process environment against the one [`Config::env_for`]
//! would export, which is exactly what `rv shell env` feeds to the shell hook.

use camino::{Utf8Path, Utf8PathBuf};
use rv_ruby::{Ruby, canonical_name::CanonicalName};

use super::{Check, ProcessEnv, Result, Section};
use crate::commands::shell::Shell;
use crate::config::Config;

const SECTION: Section = Section::Environment;

/// Variables whose expected value is computed *from* the current environment, so
/// comparing them against it would only restate what rv just read. `PATH` gets a
/// structural check instead; `MANPATH` is appended to whatever was already there.
const DERIVED_FROM_ENV: [&str; 2] = ["PATH", "MANPATH"];

/// Other Ruby version managers, recognized by the directories they put on `PATH`.
/// rv can drop the variables they export, but not their shim directories — only
/// the user can reorder their shell startup files.
///
/// Matched against a path whose separators have been normalized to `/`, so each
/// token can end in one and avoid firing on names that merely contain it.
///
/// chruby is deliberately absent: it points `PATH` straight at a Ruby install
/// rather than at shims of its own, and the directory it uses is one rv already
/// searches.
const SHIM_DIRECTORIES: [(&str, &str); 4] = [
    ("rbenv/", "rbenv"),
    ("/.rvm/", "RVM"),
    ("asdf/", "asdf"),
    ("mise/", "mise"),
];

pub(super) fn check(config: &Config, env: &ProcessEnv) -> Result<Vec<Check>> {
    let mut checks = vec![selection(config)];

    let Some(ruby) = config.best_ruby() else {
        // Without a Ruby there is no expected environment to compare against.
        checks.push(Check::skipped(
            SECTION,
            "activation",
            "no Ruby to activate yet",
        ));
        return Ok(checks);
    };

    // A blocked activation makes every downstream check report the same thing;
    // one actionable failure beats a screenful of them.
    if let Some(blocked) = activation(env, &ruby) {
        checks.push(blocked);
        return Ok(checks);
    }

    checks.extend(drift(config, env, &ruby)?);
    checks.extend(path(config, env, &ruby));

    Ok(checks)
}

/// Which Ruby rv resolves to here, and whether it is actually installed.
fn selection(config: &Config) -> Check {
    let request = config.ruby_request();

    let Some(ruby) = config.current_ruby() else {
        let reason = config.requested_ruby.explain(false);

        return if config.rubies().is_empty() {
            Check::fail(SECTION, "ruby", "no Ruby installations found")
                .suggest_command(reason, "rv ruby install")
        } else {
            Check::fail(SECTION, "ruby", format!("{request} is not installed"))
                .suggest_command(reason, format!("rv ruby install {request}"))
        };
    };

    Check::ok(
        SECTION,
        "ruby",
        format!(
            "{} — {}",
            ruby.version.canonical_name(),
            config.requested_ruby.explain(true)
        ),
    )
}

/// Whether anything blocks the per-variable checks below: no rv-managed Ruby in
/// this environment at all, or one that has since been deleted from disk.
///
/// Returns `None` when activation is healthy — the individual variables report
/// the details, so an extra passing line here would only repeat them.
///
/// rv cannot tell *who* exported these variables — the shell hook, or an
/// enclosing `rv run` — only whether they are present and current.
fn activation(env: &ProcessEnv, ruby: &Ruby) -> Option<Check> {
    let Some(root) = env.get("RUBY_ROOT") else {
        return Some(
            Check::fail(SECTION, "activation", "no Ruby active in this environment")
                .suggest_command(
                    "rv's shell integration isn't installed, or hasn't run yet in this session.",
                    setup_command(env),
                ),
        );
    };

    let root = Utf8Path::new(root);

    if rv_ruby::find_ruby_executable(root).is_some() {
        return None;
    }

    Some(
        Check::fail(
            SECTION,
            "activation",
            format!("RUBY_ROOT points at {}, which has no Ruby", shorten(root)),
        )
        .suggest_command(
            format!(
                "That Ruby was removed while this shell was running. rv would use {} instead.",
                ruby.version.canonical_name()
            ),
            refresh_command(env),
        ),
    )
}

/// Compare every variable rv would export against what this process inherited.
fn drift(config: &Config, env: &ProcessEnv, ruby: &Ruby) -> Result<Vec<Check>> {
    let (unset, set) = config.env_for(Some(ruby))?.split();

    let stale = unset
        .into_iter()
        .filter(|var| env.get(var).is_some())
        .map(|var| {
            Check::warn(SECTION, var, "set, but rv clears it").suggest_command(
                "Usually left behind by another Ruby manager. It leaks into \
                 everything rv runs.",
                format!("unset {var}"),
            )
        });

    let drifted = set
        .into_iter()
        .filter(|(var, _)| !DERIVED_FROM_ENV.contains(var))
        .map(|(var, expected)| match env.get(var) {
            Some(actual) if actual == expected => Check::ok(SECTION, var, shorten_str(actual)),
            Some(actual) => Check::fail(
                SECTION,
                var,
                format!(
                    "{} — rv expects {}",
                    shorten_str(actual),
                    shorten_str(&expected)
                ),
            )
            .suggest_command(
                "Something in this session is overriding rv's environment.",
                refresh_command(env),
            ),
            None => Check::fail(SECTION, var, "not set").suggest_command(
                format!("rv expects {}", shorten_str(&expected)),
                refresh_command(env),
            ),
        });

    Ok(stale.chain(drifted).collect())
}

/// `PATH` has to contain rv's directories, and `ruby` has to resolve to the Ruby
/// rv picked rather than to a shim or a system install sitting in front of it.
fn path(config: &Config, env: &ProcessEnv, ruby: &Ruby) -> Vec<Check> {
    let entries = env.path();

    let missing: Vec<_> = config
        .path_prefix_for(ruby)
        .into_iter()
        .filter(|dir| !entries.contains(dir))
        .map(|dir| shorten(&dir))
        .collect();

    let prefix = if missing.is_empty() {
        Check::ok(SECTION, "PATH", "rv's directories are present")
    } else {
        Check::fail(SECTION, "PATH", format!("missing {}", missing.join(", "))).suggest_command(
            "Gem executables installed for this Ruby won't be found.",
            refresh_command(env),
        )
    };

    vec![prefix, resolves_to(&entries, ruby, env)]
}

fn resolves_to(entries: &[Utf8PathBuf], ruby: &Ruby, env: &ProcessEnv) -> Check {
    let expected = ruby.executable_path();

    let Some(found) = which_ruby(entries) else {
        return Check::fail(SECTION, "ruby on PATH", "`ruby` is not on PATH").suggest_command(
            format!("rv expects it at {}", shorten(&expected)),
            refresh_command(env),
        );
    };

    if found == expected {
        return Check::ok(SECTION, "ruby on PATH", shorten(&found));
    }

    let check = Check::fail(
        SECTION,
        "ruby on PATH",
        format!("resolves to {}", shorten(&found)),
    );

    match competing_manager(&found) {
        Some(manager) => check.suggest(format!(
            "{manager} is ahead of rv on PATH. Remove its setup from your shell \
             startup files, or move rv's after it.",
        )),
        None => check.suggest_command(
            format!("rv expects {}", shorten(&expected)),
            refresh_command(env),
        ),
    }
}

/// The first `ruby` on `PATH`, resolved the way a shell would resolve it.
fn which_ruby(entries: &[Utf8PathBuf]) -> Option<Utf8PathBuf> {
    entries
        .iter()
        .flat_map(|dir| {
            rv_ruby::ruby_executable_names()
                .iter()
                .map(move |name| dir.join(name))
        })
        .find(|candidate| candidate.is_file())
}

fn competing_manager(path: &Utf8Path) -> Option<&'static str> {
    let path = path.as_str().to_ascii_lowercase().replace('\\', "/");

    SHIM_DIRECTORIES
        .iter()
        .find(|(token, _)| path.contains(token))
        .map(|(_, manager)| *manager)
}

/// The shell rv was launched from, if rv knows how to talk to it.
fn shell(env: &ProcessEnv) -> Option<Shell> {
    Shell::from_path(Utf8Path::new(env.get("SHELL")?))
}

/// How to install rv's shell integration from scratch.
fn setup_command(env: &ProcessEnv) -> String {
    match shell(env) {
        Some(shell) => format!("rv shell {shell}"),
        None => "rv shell <your shell>".to_string(),
    }
}

/// How to re-apply rv's environment to the *current* session.
fn refresh_command(env: &ProcessEnv) -> String {
    match shell(env) {
        Some(shell @ (Shell::Zsh | Shell::Bash)) => format!("eval \"$(rv shell env {shell})\""),
        Some(Shell::Fish) => "rv shell env fish | source".to_string(),
        Some(Shell::Nu) => "rv shell env nu | from json | load-env".to_string(),
        Some(Shell::PowerShell) => {
            "rv shell env powershell | Out-String | Invoke-Expression".to_string()
        }
        None => "rv shell env <your shell>".to_string(),
    }
}

fn shorten(path: &Utf8Path) -> String {
    rv_dirs::unexpand(path)
}

/// Values like `GEM_PATH` hold several paths at once, so shorten the raw string.
fn shorten_str(value: &str) -> String {
    rv_dirs::unexpand(Utf8Path::new(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_shim_directories() {
        for (path, expected) in [
            ("/home/dev/.rbenv/shims/ruby", Some("rbenv")),
            ("/home/dev/.rvm/rubies/ruby-3.4.1/bin/ruby", Some("RVM")),
            ("/home/dev/.asdf/shims/ruby", Some("asdf")),
            ("/home/dev/.local/share/mise/shims/ruby", Some("mise")),
            ("/usr/bin/ruby", None),
            // Separators are normalized, so Windows paths match too.
            ("C:\\Users\\dev\\.rbenv\\shims\\ruby.exe", Some("rbenv")),
            // A name that merely contains a manager's is not a match.
            ("/home/wasdfg/bin/ruby", None),
        ] {
            assert_eq!(
                competing_manager(Utf8Path::new(path)),
                expected,
                "for {path}"
            );
        }
    }

    #[test]
    fn setup_command_follows_the_shell() {
        let env = ProcessEnv::from_iter([("SHELL", "/usr/bin/fish")]);
        assert_eq!(setup_command(&env), "rv shell fish");
        assert_eq!(refresh_command(&env), "rv shell env fish | source");
    }

    #[test]
    fn setup_command_stays_generic_for_unknown_shells() {
        let env = ProcessEnv::from_iter([("SHELL", "/bin/ksh")]);
        assert_eq!(setup_command(&env), "rv shell <your shell>");

        assert_eq!(
            setup_command(&ProcessEnv::default()),
            "rv shell <your shell>"
        );
    }
}
