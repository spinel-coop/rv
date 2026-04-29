use clap::Args;
use std::io;
use std::{borrow::Cow, collections::BTreeMap};
use tabled::{
    Table,
    settings::{Panel, Span, Style, style::HorizontalLine, themes::BorderCorrection},
};

use anstream::println;
use owo_colors::OwoColorize;
use rv_ruby::{
    RemoteRuby, Ruby, canonical_name::CanonicalName, request::RubyRequest, version::RubyVersion,
};
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
    ruby: RubyEntry,
    active: bool,
    eol_date: String,
    #[serde(skip)]
    color: bool,
}

impl JsonRubyEntry {
    fn no_color(&mut self) {
        self.color = false;
    }
}

#[derive(Serialize, Debug)]
#[cfg_attr(test, derive(PartialEq))]
enum RubyEntry {
    Installed(Ruby),
    Remote(RemoteRuby),
}

impl RubyEntry {
    pub fn canonical_name(&self) -> String {
        match self {
            Self::Installed(ruby) => ruby.version.canonical_name(),
            Self::Remote(remote_ruby) => remote_ruby.version.canonical_name(),
        }
    }
}

impl tabled::Tabled for JsonRubyEntry {
    const LENGTH: usize = 2;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        let canonical_name = self.ruby.canonical_name();

        let name = if self.active {
            format!("* {canonical_name}")
        } else {
            format!("  {canonical_name}")
        };

        let installed = match &self.ruby {
            RubyEntry::Installed(ruby) => {
                let short_executable_path = rv_dirs::unexpand(&ruby.executable_path());

                if self.color {
                    short_executable_path.cyan().to_string().into()
                } else {
                    short_executable_path.into()
                }
            }
            RubyEntry::Remote(_) => {
                if self.color {
                    "[available]".dimmed().to_string().into()
                } else {
                    "[available]".to_string().into()
                }
            }
        };

        vec![name.into(), installed, self.eol_date.to_string().into()]
    }

    fn headers() -> Vec<Cow<'static, str>> {
        vec!["Version".into(), "Installed".into(), "EOL".into()]
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
    let mut active_ruby = false;

    // Might have multiple installed rubies with the same version (e.g., "ruby-3.2.0" and "mruby-3.2.0").
    let mut rubies_map: BTreeMap<RubyVersion, Vec<JsonRubyEntry>> = BTreeMap::new();

    for ruby in installed_rubies.into_iter().rev() {
        let eol_date = crate::ruby_eol::eol_information_for(&ruby.version, &config.cache)
            .await
            .unwrap()
            .unwrap();

        rubies_map.entry(ruby.version.clone()).or_default().insert(
            0,
            JsonRubyEntry {
                active: active(&mut active_ruby, &ruby.version, &requested),
                ruby: RubyEntry::Installed(ruby),
                eol_date: eol_date.eol_from,
                color: true,
            },
        );
    }

    let active_installed = active_ruby;

    if !version_filter.installed_only {
        let remote_rubies = config.remote_rubies().await;

        let selected_remote_rubies = if version_filter.all {
            remote_rubies.clone()
        } else {
            latest_patch_version(&remote_rubies)
        };

        // Add selected remote rubies that are not already installed to the list
        for ruby in selected_remote_rubies.into_iter().rev() {
            let eol_date = crate::ruby_eol::eol_information_for(&ruby.version, &config.cache)
                .await
                .unwrap()
                .unwrap();

            rubies_map
                .entry(ruby.version.clone())
                .or_insert(vec![JsonRubyEntry {
                    active: active(&mut active_ruby, &ruby.version, &requested),
                    ruby: RubyEntry::Remote(ruby),
                    eol_date: eol_date.eol_from,
                    color: true,
                }]);
        }

        if !active_ruby {
            let ruby = requested.find_match_in(&remote_rubies);

            if let Some(ref ruby) = ruby {
                let eol_date = crate::ruby_eol::eol_information_for(&ruby.version, &config.cache)
                    .await
                    .unwrap()
                    .unwrap();

                rubies_map
                    .entry(ruby.version.clone())
                    .or_insert(vec![JsonRubyEntry {
                        ruby: RubyEntry::Remote(ruby.clone()),
                        active: true,
                        eol_date: eol_date.eol_from,
                        color: true,
                    }]);
            };
        };

        if rubies_map.is_empty() && format == OutputFormat::Text {
            warn!("No rubies found for your platform.");
            return Ok(());
        }
    }

    // Create entries for output
    let entries: Vec<JsonRubyEntry> = rubies_map.into_values().flatten().collect();

    let explanation = config.requested_ruby.explain(active_installed);

    print_entries(entries, format, no_color, &explanation)
}

fn active(active_set: &mut bool, version: &RubyVersion, requested: &RubyRequest) -> bool {
    if *active_set {
        return false;
    }

    let should_activate = version.satisfies(requested);

    *active_set |= should_activate;

    should_activate
}

fn latest_patch_version(remote_rubies: &Vec<RemoteRuby>) -> Vec<RemoteRuby> {
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
    let mut available_rubies: BTreeMap<NonPatchRelease, RemoteRuby> = BTreeMap::new();
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
    explanation: &String,
) -> Result<()> {
    match format {
        OutputFormat::Text => {
            if no_color {
                for e in entries.iter_mut() {
                    e.no_color();
                }
            }
            let size = entries.len() + 1;
            let mut table = Table::new(entries);
            let style = Style::sharp().horizontals([
                (1, HorizontalLine::full('─', '┼', '├', '┤')),
                (size, HorizontalLine::full('─', '┼', '├', '┤')),
            ]);
            table
                .with(Panel::footer(explanation))
                .with(style)
                .modify((size, 0), Span::column(0))
                .with(BorderCorrection::span());

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
    use rv_ruby::version::RubyVersion;
    use std::str::FromStr as _;

    fn global_args() -> Result<GlobalArgs> {
        let root_dir = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        let ruby_dir = root_dir.join("opt/rubies");
        fs_err::create_dir_all(&ruby_dir)?;

        let cache_args = CacheArgs {
            no_cache: false,
            cache_dir: None,
        };

        let global_args = GlobalArgs {
            ruby_dir: [ruby_dir].to_vec(),
            cache_args,
            offline: false,
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

    fn ruby(version: &str) -> RemoteRuby {
        let version = RubyVersion::from_str(version).unwrap();
        let version_str = version.to_string();
        RemoteRuby {
            key: format!("{version_str}-macos-aarch64"),
            version,
            arch: "aarch64".into(),
            os: "macos".into(),
        }
    }

    #[test]
    fn test_latest_patch_version() {
        struct Test {
            name: &'static str,
            input: Vec<RemoteRuby>,
            expected: Vec<RemoteRuby>,
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
