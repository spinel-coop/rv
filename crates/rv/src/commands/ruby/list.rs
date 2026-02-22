use clap::Args;
use std::io;
use std::{borrow::Cow, collections::BTreeMap};
use tabled::{Table, settings::Style};

use anstream::println;
use owo_colors::OwoColorize;
use rv_ruby::{Ruby, version::ReleasedRubyVersion};
use serde::Serialize;
use tracing::{info, warn};

use crate::{GlobalArgs, config::Config, output_format::OutputFormat};

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
#[derive(Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct JsonRubyEntry {
    #[serde(flatten)]
    details: Ruby,
    installed: bool,
    active: bool,
    #[serde(skip)]
    color: bool,
}

impl JsonRubyEntry {
    fn installed(details: Ruby, active_ruby: &Option<Ruby>) -> Self {
        Self::new(details, true, active_ruby)
    }

    fn available(details: Ruby, active_ruby: &Option<Ruby>) -> Self {
        Self::new(details, false, active_ruby)
    }

    fn new(details: Ruby, installed: bool, active_ruby: &Option<Ruby>) -> Self {
        JsonRubyEntry {
            active: active_ruby.as_ref().is_some_and(|a| a == &details),
            installed,
            details,
            color: true,
        }
    }

    fn no_color(&mut self) {
        self.color = false;
    }
}

impl tabled::Tabled for JsonRubyEntry {
    const LENGTH: usize = 3;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        let name = if self.active {
            format!("* {}", self.details.version)
        } else {
            format!("  {}", self.details.version)
        };

        let installed = if self.installed {
            if self.color {
                "[installed]".green().to_string().into()
            } else {
                "[installed]".to_string().into()
            }
        } else if self.color {
            "[available]".dimmed().to_string().into()
        } else {
            "[available]".to_string().into()
        };
        let path = if self.installed {
            if self.color {
                self.details.executable_path().cyan().to_string().into()
            } else {
                self.details.executable_path().to_string().into()
            }
        } else {
            "".into()
        };
        vec![name.into(), installed, path]
    }

    fn headers() -> Vec<Cow<'static, str>> {
        vec!["Version".into(), "Installed".into(), "Path".into()]
    }
}

#[derive(Args)]
#[group(required = false, multiple = false)]
pub struct VersionFilter {
    /// List all versions (Including outdated)
    #[arg(long, help_heading = "Filter Options")]
    all: bool,

    /// List only installed versions
    #[arg(long, help_heading = "Filter Options")]
    installed_only: bool,
}

/// Lists the available and installed rubies.
pub(crate) async fn list(
    global_args: &GlobalArgs,
    format: OutputFormat,
    version_filter: VersionFilter,
    no_color: bool,
) -> Result<()> {
    let config = Config::new(global_args, None)?;

    let installed_rubies = config.rubies();

    if version_filter.installed_only && installed_rubies.is_empty() && format == OutputFormat::Text
    {
        warn!("No Ruby installations found.");
        info!("Try installing Ruby with 'rv ruby install <version>'");
        return Ok(());
    }

    let requested = config.ruby_request();
    let mut active_ruby = requested.find_match_in(&installed_rubies);

    // Might have multiple installed rubies with the same version (e.g., "ruby-3.2.0" and "mruby-3.2.0").
    let mut rubies_map: BTreeMap<ReleasedRubyVersion, Vec<JsonRubyEntry>> = BTreeMap::new();

    for ruby in installed_rubies {
        rubies_map
            .entry(ruby.version.clone())
            .or_default()
            .push(JsonRubyEntry::installed(ruby, &active_ruby));
    }

    if !version_filter.installed_only {
        let remote_rubies = config.remote_rubies().await;

        let selected_remote_rubies = if version_filter.all {
            remote_rubies.clone()
        } else {
            latest_patch_version(&remote_rubies)
        };

        active_ruby = active_ruby.or_else(|| requested.find_match_in(&selected_remote_rubies));

        // Add selected remote rubies that are not already installed to the list
        for ruby in selected_remote_rubies {
            rubies_map
                .entry(ruby.version.clone())
                .or_insert(vec![JsonRubyEntry::available(ruby, &active_ruby)]);
        }

        let insert_requested_if_available = || {
            let ruby = requested.find_match_in(&remote_rubies);

            if ruby.is_some() {
                let details = ruby.clone().unwrap();

                rubies_map
                    .entry(details.version.clone())
                    .or_insert(vec![JsonRubyEntry {
                        details,
                        installed: false,
                        active: true,
                        color: true,
                    }]);
            };

            ruby
        };

        active_ruby.or_else(insert_requested_if_available);

        if rubies_map.is_empty() && format == OutputFormat::Text {
            warn!("No rubies found for your platform.");
            return Ok(());
        }
    }

    // Create entries for output
    let entries: Vec<JsonRubyEntry> = rubies_map.into_values().flatten().collect();

    print_entries(entries, format, no_color)
}

fn latest_patch_version(remote_rubies: &Vec<Ruby>) -> Vec<Ruby> {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct NonPatchRelease {
        engine: rv_ruby::engine::RubyEngine,
        major: rv_ruby::request::VersionPart,
        minor: rv_ruby::request::VersionPart,
    }

    impl From<ReleasedRubyVersion> for NonPatchRelease {
        fn from(value: ReleasedRubyVersion) -> Self {
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

fn print_entries(
    mut entries: Vec<JsonRubyEntry>,
    format: OutputFormat,
    no_color: bool,
) -> Result<()> {
    match format {
        OutputFormat::Text => {
            if no_color {
                for e in entries.iter_mut() {
                    e.no_color();
                }
            }
            let mut table = Table::new(entries);
            table.with(Style::sharp());
            println!("{table}");
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(io::stdout(), &entries)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GlobalArgs;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use rv_cache::CacheArgs;
    use rv_ruby::version::ReleasedRubyVersion;
    use std::str::FromStr as _;

    fn global_args() -> Result<GlobalArgs> {
        let root_dir = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        let ruby_dir = root_dir.join("opt/rubies");
        fs_err::create_dir_all(&ruby_dir)?;
        let current_exe = root_dir.join("bin").join("rv");

        let cache_args = CacheArgs {
            no_cache: false,
            cache_dir: None,
        };

        let global_args = GlobalArgs {
            ruby_dir: [ruby_dir].to_vec(),
            current_exe: Some(current_exe),
            cache_args,
        };

        Ok(global_args)
    }

    #[tokio::test]
    async fn test_list() {
        let global_args = global_args().unwrap();
        let version_filter = VersionFilter {
            all: false,
            installed_only: false,
        };
        list(&global_args, OutputFormat::Text, version_filter, true)
            .await
            .unwrap();
    }

    fn ruby(version: &str) -> Ruby {
        let version = ReleasedRubyVersion::from_str(version).unwrap();
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
