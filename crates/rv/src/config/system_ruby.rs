use camino::Utf8PathBuf;
use tracing::{debug, instrument};

use rv_ruby::canonical_name::CanonicalName;
use rv_ruby::{EnvProvider, Ruby, SystemEnv};

use super::Config;

/// System ruby discovery (Debian `/usr/bin/ruby`, `which ruby`, etc.).
///
/// These are surfaced via `rv ruby list` (and `find`) for visibility, but are
/// not managed by `rv`: they cannot be installed, uninstalled, or modified
/// through `rv`. They are also never used as the destination of `rv ruby install`.
impl Config {
    /// Discover Ruby executables on PATH.
    ///
    /// Returns Rubies with `managed = false`, deduplicated against installed
    /// Rubies in `ruby_dirs` (same canonical name + same executable path).
    ///
    /// Only PATH is consulted. The Debian/Ubuntu system Ruby at
    /// `/usr/bin/ruby` is reached through PATH; hardcoding absolute paths
    /// would leak the host's installed Ruby into containerized tests and
    /// CI environments that did not opt in.
    #[instrument(skip_all, level = "trace")]
    pub fn discover_system_rubies_with<E: EnvProvider>(&self, provider: &E) -> Vec<Ruby> {
        let mut candidates: Vec<Utf8PathBuf> = Vec::new();

        if let Some(path_var) = provider.get_var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                // Probe both `<dir>/ruby` (PATH entries like `/usr/bin`) and
                // `<dir>/bin/ruby` (PATH entries like `/opt/rubies`). The
                // convention varies — Debian `/usr/bin/ruby` lives at PATH root,
                // while rvm/rbenv shims put `<version>/bin/ruby` under a single
                // PATH entry.
                let bin_dir = dir.join("bin");
                for entry in [dir.as_path(), bin_dir.as_path()] {
                    for name in ["ruby", "ruby3", "ruby2"] {
                        if let Some(found) = probe_path_entry(entry, name) {
                            candidates.push(found);
                        }
                    }
                }
            }
        }

        let mut seen: Vec<Utf8PathBuf> = Vec::new();
        let mut rubies: Vec<Ruby> = Vec::new();
        for exec in candidates {
            let canonical = exec.canonicalize_utf8().unwrap_or_else(|_| exec.clone());
            if seen.iter().any(|p| p == &canonical) {
                continue;
            }
            seen.push(canonical);

            // Skip any executable that lives under a managed ruby_dir.
            if self.ruby_dirs.iter().any(|d| exec.starts_with(d)) {
                continue;
            }

            match Ruby::from_executable_path(exec.clone()) {
                Ok(ruby) => {
                    if ruby.is_valid() {
                        rubies.push(ruby);
                    } else {
                        debug!("System ruby at {:?} is invalid", exec);
                    }
                }
                Err(err) => {
                    debug!("Failed to probe system ruby at {:?}: {err}", exec);
                }
            }
        }

        rubies.sort();
        rubies
    }

    /// Production wrapper using the live process environment.
    pub fn discover_system_rubies(&self) -> Vec<Ruby> {
        self.discover_system_rubies_with(&SystemEnv)
    }

    /// Like [`Self::discover_system_rubies_with`] but applies `predicate` to the
    /// canonical version string of each candidate. Used by `uninstall` to
    /// detect the version-specific match.
    #[instrument(skip_all, level = "trace")]
    pub fn discover_system_rubies_filtered_with<E: EnvProvider, F>(
        &self,
        provider: &E,
        predicate: &F,
    ) -> Vec<Ruby>
    where
        F: Fn(&str) -> bool,
    {
        self.discover_system_rubies_with(provider)
            .into_iter()
            .filter(|r| {
                let name = r.version.canonical_name();
                predicate(&name) || predicate(r.path.as_str())
            })
            .collect()
    }

    /// Production wrapper for [`Self::discover_system_rubies_filtered_with`].
    pub fn discover_system_rubies_filtered<F>(&self, predicate: &F) -> Vec<Ruby>
    where
        F: Fn(&str) -> bool,
    {
        self.discover_system_rubies_filtered_with(&SystemEnv, predicate)
    }
}

/// Look for `name` in `dir`. Only returns the path if it is a regular file
/// (executable bit on Unix). Skips directories to avoid false positives like
/// `/usr/bin/ruby-foo` when probing for `ruby`.
fn probe_path_entry(dir: &std::path::Path, name: &str) -> Option<Utf8PathBuf> {
    let path = dir.join(name);
    if !path.is_file() {
        return None;
    }
    Utf8PathBuf::try_from(path).ok()
}

#[cfg(test)]
#[cfg(not(windows))]
mod tests {
    use std::fs;
    use tempfile::TempDir;

    use crate::config::test_support::{FakeEnv, make_mock_ruby_shim};

    use super::super::Config;
    use super::*;

    // Writes a fake shim of a Ruby file to test if it shows up.
    #[test]
    fn probe_path_entry_finds_executable() {
        let tmp = TempDir::new().unwrap();
        let bin = tmp.path().join("ruby");
        fs::write(&bin, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&bin).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&bin, perms).unwrap();
        }
        assert!(probe_path_entry(tmp.path(), "ruby").is_some());
        assert!(probe_path_entry(tmp.path(), "nonexistent").is_none());
    }

    #[test]
    fn probe_path_entry_skips_directories() {
        let tmp = TempDir::new().unwrap();
        // A directory named "ruby" must not be picked up — would otherwise
        // trigger false positives during PATH probing.
        fs::create_dir(tmp.path().join("ruby")).unwrap();
        assert!(probe_path_entry(tmp.path(), "ruby").is_none());
    }

    #[test]
    fn discover_system_rubies_returns_empty_when_path_unset() {
        let config = Config::new_dummy();
        let env = FakeEnv::new();
        let result = config.discover_system_rubies_with(&env);
        assert!(result.is_empty());
    }

    #[test]
    fn discover_system_rubies_finds_ruby_on_path() {
        let tmp = TempDir::new().unwrap();
        make_mock_ruby_shim(tmp.path());

        let config = Config::new_dummy();
        let env = FakeEnv::new().with("PATH", tmp.path().to_str().unwrap());
        let result = config.discover_system_rubies_with(&env);

        assert_eq!(result.len(), 1, "expected 1 system ruby, got: {result:?}");
        assert!(!result[0].managed, "system ruby must be unmanaged");
    }

    #[test]
    fn discover_system_rubies_probes_bin_subdir() {
        // PATH entry points to a dir whose ruby shim lives at `<dir>/bin/ruby`.
        // Probe code checks both `<dir>/ruby` and `<dir>/bin/ruby`.
        let tmp = TempDir::new().unwrap();
        make_mock_ruby_shim(tmp.path());

        let config = Config::new_dummy();
        let env = FakeEnv::new().with("PATH", tmp.path().to_str().unwrap());
        let result = config.discover_system_rubies_with(&env);

        assert_eq!(
            result.len(),
            1,
            "<dir>/bin/ruby must be probed, got: {result:?}",
        );
    }

    #[test]
    fn discover_system_rubies_dedupes_canonicalized_paths() {
        // Two PATH entries whose canonical paths resolve to the same file
        // must produce a single result.
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        make_mock_ruby_shim(tmp1.path());
        // tmp2/bin -> tmp1/bin
        #[cfg(unix)]
        std::os::unix::fs::symlink(tmp1.path().join("bin"), tmp2.path().join("bin")).unwrap();

        let path_value = format!(
            "{}:{}",
            tmp1.path().to_str().unwrap(),
            tmp2.path().to_str().unwrap()
        );
        let config = Config::new_dummy();
        let env = FakeEnv::new().with("PATH", &path_value);
        let result = config.discover_system_rubies_with(&env);

        assert_eq!(
            result.len(),
            1,
            "canonicalized dedupe should yield 1, got: {result:?}",
        );
    }

    #[test]
    fn discover_system_rubies_skips_managed_dirs() {
        let config = Config::new_dummy();
        let managed = &config.ruby_dirs[0];
        fs::create_dir_all(managed.as_std_path()).unwrap();
        let exec = managed.join("ruby");
        fs::write(&exec, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&exec).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exec, perms).unwrap();
        }

        let env = FakeEnv::new().with("PATH", managed.as_str());
        let result = config.discover_system_rubies_with(&env);

        assert!(
            result.is_empty(),
            "exec under ruby_dirs must be skipped, got: {result:?}",
        );
    }

    #[test]
    fn discover_system_rubies_filtered_applies_predicate() {
        let tmp = TempDir::new().unwrap();
        make_mock_ruby_shim(tmp.path());

        let config = Config::new_dummy();
        let env = FakeEnv::new().with("PATH", tmp.path().to_str().unwrap());

        let matching =
            config.discover_system_rubies_filtered_with(&env, &|s: &str| s.contains("3.0.1"));
        assert_eq!(
            matching.len(),
            1,
            "predicate matching canonical_name must yield the ruby",
        );

        let none = config.discover_system_rubies_filtered_with(&env, &|_: &str| false);
        assert!(none.is_empty(), "predicate rejecting all must yield none");
    }

    #[test]
    fn discover_system_rubies_finds_ruby3_and_ruby2() {
        let tmp = TempDir::new().unwrap();
        // `is_valid()` checks `<path>/bin/ruby` exists, so create a
        // placeholder `ruby` alongside `ruby3`/`ruby2` to satisfy the gate.
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        for name in ["ruby", "ruby3", "ruby2"] {
            let exec = bin.join(name);
            let script = "\
#!/bin/bash
 echo \"ruby\"\n echo \"3.0.1\"\n echo \"aarch64-darwin23\"\n echo \"aarch64\"\n echo \"darwin23\"\n echo \"\"\n";
            fs::write(&exec, script).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&exec).unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&exec, perms).unwrap();
            }
        }

        let config = Config::new_dummy();
        let env = FakeEnv::new().with("PATH", tmp.path().to_str().unwrap());
        let result = config.discover_system_rubies_with(&env);

        // All three names are probed; `ruby3`/`ruby2` need `bin/ruby` for `is_valid`.
        assert!(
            result.len() >= 2,
            "expected at least ruby3 and ruby2, got: {result:?}"
        );
        assert!(result.iter().all(|r| !r.managed));
    }

    #[test]
    fn discover_system_rubies_ignores_empty_path_segments() {
        let tmp = TempDir::new().unwrap();
        make_mock_ruby_shim(tmp.path());

        let config = Config::new_dummy();
        let path_with_empty = format!("::{}::", tmp.path().to_str().unwrap());
        let env = FakeEnv::new().with("PATH", &path_with_empty);
        let result = config.discover_system_rubies_with(&env);

        assert_eq!(
            result.len(),
            1,
            "empty PATH segments should be ignored, got: {result:?}"
        );
    }
}
