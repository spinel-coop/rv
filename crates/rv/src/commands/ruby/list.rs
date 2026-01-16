use std::collections::BTreeMap;
use std::io;

use anstream::println;
use owo_colors::OwoColorize;
use rv_ruby::{Ruby, version::RubyVersion};
use serde::Serialize;
use tracing::{info, warn};

use crate::config::Config;

#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    VersionError(#[from] rv_ruby::request::RequestError),
    #[error(transparent)]
    RubyError(#[from] rv_ruby::RubyError),
}

type Result<T> = miette::Result<T, Error>;

// Struct for JSON output and maintaing the list of installed/active rubies
#[derive(Serialize)]
#[cfg_attr(test, derive(Debug, PartialEq))]
struct RubyEntry {
    #[serde(flatten)]
    ruby: Ruby,
    installed: bool,
    active: bool,
}

/// Lists the available and installed rubies.
pub async fn list(config: &Config, format: OutputFormat, installed_only: bool) -> Result<()> {
    let request = config.ruby_request();
    let rubies: Vec<Ruby> = config.rubies();
    let active_ruby = request.find_match_in(&rubies);

    // Might have multiple installed rubies with the same version (e.g., "ruby-3.2.0" and "mruby-3.2.0").
    let mut rubies_map: BTreeMap<String, Vec<RubyEntry>> = BTreeMap::new();

    for ruby in rubies {
        rubies_map
            .entry(ruby.display_name())
            .or_default()
            .push(RubyEntry {
                ruby: ruby.clone(),
                active: active_ruby.as_ref().is_some_and(|r| *r == ruby),
                installed: true,
            });
    }

    if !installed_only {
        let remote_rubies = config.remote_rubies().await;
        let platform_rubies = latest_patch_version(&remote_rubies);

        let active_ruby = active_ruby.or_else(|| request.find_match_in(&platform_rubies));

        for ruby in platform_rubies {
            rubies_map
                .entry(ruby.display_name())
                .or_insert(vec![RubyEntry {
                    ruby: ruby.clone(),
                    active: active_ruby.as_ref().is_some_and(|r| *r == ruby),
                    installed: false,
                }]);
        }
    }

    let entries: Vec<RubyEntry> = rubies_map.into_values().flatten().collect();

    match format {
        OutputFormat::Json => serde_json::to_writer_pretty(io::stdout(), entries.as_slice())?,
        OutputFormat::Text => {
            if entries.iter().all(|r| !r.installed) {
                warn!("No Ruby installations found.");
                info!("Try installing Ruby with 'rv ruby install <version>'");
            } else if entries.is_empty() {
                warn!("No rubies found for your platform.");
            } else {
                print_entries(&entries);
            }
        }
    }

    Ok(())
}

fn latest_patch_version(remote_rubies: &Vec<Ruby>) -> Vec<Ruby> {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct NonPatchRelease {
        engine: rv_ruby::engine::RubyEngine,
        major: rv_ruby::request::VersionPart,
        minor: rv_ruby::request::VersionPart,
    }

    impl From<RubyVersion> for NonPatchRelease {
        fn from(value: RubyVersion) -> Self {
            Self {
                engine: value.engine,
                major: value.major,
                minor: value.minor,
            }
        }
    }
    let mut available_rubies: BTreeMap<NonPatchRelease, Ruby> = BTreeMap::new();
    for ruby in remote_rubies {
        // Skip 3.5 series since they only include pre-releases
        if ruby.version.major == 3 && ruby.version.minor == 5 {
            continue;
        }

        let key = NonPatchRelease::from(ruby.version.clone());
        let skip = available_rubies
            .get(&key)
            .map(|other| other.version > ruby.version)
            .unwrap_or_default();
        if !skip {
            available_rubies.insert(key, ruby.clone());
        }
    }
    available_rubies.into_values().collect()
}

fn print_entries(entries: &[RubyEntry]) -> () {
    let width = entries
        .iter()
        .map(|e| e.ruby.display_name().len())
        .max()
        .unwrap_or(0);

    for entry in entries {
        let marker = if entry.active { "*" } else { " " };
        let name = entry.ruby.display_name();

        if entry.installed {
            println!(
                "{marker} {name:width$} {} {}",
                "[installed]".green(),
                entry.ruby.executable_path().cyan()
            )
        } else {
            println!("{marker} {name:width$} {}", "[available]".dimmed())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use indexmap::indexset;
    use rv_ruby::{request::Source, version::RubyVersion};
    use std::str::FromStr as _;

    fn test_config() -> Result<Config> {
        let root = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        let ruby_dir = root.join("opt/rubies");
        fs_err::create_dir_all(&ruby_dir)?;
        let current_dir = root.join("project");
        fs_err::create_dir_all(&current_dir)?;

        let config = Config {
            ruby_dirs: indexset![ruby_dir],
            current_exe: root.join("bin").join("rv"),
            requested_ruby: Some(("3.5.0".parse().unwrap(), Source::Other)),
            current_dir,
            cache: rv_cache::Cache::temp().unwrap(),
            root,
        };

        Ok(config)
    }
    #[tokio::test]
    async fn test_list() {
        let config = test_config().unwrap();
        list(&config, OutputFormat::Text, false).await.unwrap();
    }

    fn ruby(version: &str) -> Ruby {
        let version = RubyVersion::from_str(version).unwrap();
        let version_str = version.to_string();
        Ruby {
            key: format!("{version_str}-macos-aarch64"),
            version,
            path: Utf8PathBuf::from(format!(
                "https://github.com/spinel-coop/rv-ruby/releases/download/latest/{version_str}.arm64_linux.tar.gz"
            )),
            managed: false,
            symlink: None,
            arch: "aarch64".into(),
            os: "macos".into(),
            gem_root: None,
        }
    }

    #[test]
    fn test_latest_patch_version() {
        struct Test {
            name: &'static str,
            input: Vec<Ruby>,
            expected: Vec<Ruby>,
        }

        let tests = vec![
            Test {
                name: "prefers_highest_patch_per_minor",
                input: vec![
                    ruby("ruby-3.2.0"),
                    ruby("ruby-3.1.5"),
                    ruby("ruby-3.2.2"),
                    ruby("ruby-3.1.6"),
                ],
                expected: vec![ruby("ruby-3.1.6"), ruby("ruby-3.2.2")],
            },
            Test {
                name: "prefers_latest_prerelease_when_all_patch_are_the_same",
                input: vec![
                    ruby("ruby-3.2.0-preview1"),
                    ruby("ruby-3.2.0-rc1"),
                    ruby("ruby-3.2.0-preview3"),
                ],
                expected: vec![ruby("ruby-3.2.0-rc1")],
            },
            Test {
                name: "prefers_stable_release_over_any_prerelease",
                input: vec![
                    ruby("ruby-3.2.0-preview1"),
                    ruby("ruby-3.2.0"),
                    ruby("ruby-3.2.0-preview3"),
                ],
                expected: vec![ruby("ruby-3.2.0")],
            },
            Test {
                name: "respects_engine_boundaries",
                input: vec![
                    ruby("jruby-9.4.12.0"),
                    ruby("ruby-3.3.1"),
                    ruby("jruby-9.4.13.1"),
                    ruby("jruby-9.4.13.0"),
                    ruby("ruby-3.3.2"),
                ],
                expected: vec![ruby("ruby-3.3.2"), ruby("jruby-9.4.13.1")],
            },
        ];

        for Test {
            name,
            input,
            expected,
        } in tests
        {
            let actual = latest_patch_version(&input);
            assert_eq!(
                actual, expected,
                "Failed test {name}, got {actual:?} but expected {expected:?}"
            );
        }
    }
}
