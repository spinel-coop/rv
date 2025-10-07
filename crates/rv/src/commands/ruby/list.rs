use std::collections::BTreeMap;
use std::io;
use std::time::{Duration, SystemTime};

use anstream::println;
use camino::Utf8PathBuf;
use current_platform::CURRENT_PLATFORM;
use fs_err as fs;
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use regex::Regex;
use rv_ruby::Ruby;
use rv_ruby::request::RubyRequest;
use rv_ruby::{Asset, Release};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::config::Config;

// Use GitHub's TTL, but don't re-check more than every 60 seconds.
const MINIMUM_CACHE_TTL: Duration = Duration::from_secs(60);

static ARCH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"ruby-[\d\.]+\.(?P<arch>[a-zA-Z0-9_]+)\.tar\.gz").unwrap());

static PARSE_MAX_AGE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"max-age=(\d+)").unwrap());

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
    #[error("Failed to fetch available ruby versions from GitHub")]
    RequestError(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    VersionError(#[from] rv_ruby::request::RequestError),
    #[error(transparent)]
    RubyError(#[from] rv_ruby::RubyError),
}

type Result<T> = miette::Result<T, Error>;

// Updated struct to hold ETag and calculated expiry time
#[derive(Serialize, Deserialize, Debug)]
struct CachedRelease {
    expires_at: SystemTime,
    etag: Option<String>,
    release: Release,
}

// Struct for JSON output and maintaing the list of installed/active rubies
#[derive(Serialize)]
struct JsonRubyEntry {
    #[serde(flatten)]
    details: Ruby,
    installed: bool,
    active: bool,
}

/// Parses the `max-age` value from a `Cache-Control` header.
fn parse_max_age(header: &str) -> Option<Duration> {
    PARSE_MAX_AGE_REGEX
        .captures(header)
        .and_then(|caps| caps.get(1))
        .and_then(|age| age.as_str().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Parses the OS and architecture from the arch part of the asset name.
fn parse_arch_str(arch_str: &str) -> (&'static str, &'static str) {
    match arch_str {
        "arm64_sonoma" => ("macos", "aarch64"),
        "x86_64_linux" => ("linux", "x86_64"),
        "arm64_linux" => ("linux", "aarch64"),
        _ => ("unknown", "unknown"),
    }
}

fn current_platform_arch_str() -> &'static str {
    let platform =
        std::env::var("RV_TEST_PLATFORM").unwrap_or_else(|_| CURRENT_PLATFORM.to_string());

    match platform.as_str() {
        "aarch64-apple-darwin" => "arm64_sonoma",
        "x86_64-unknown-linux-gnu" => "x86_64_linux",
        "aarch64-unknown-linux-gnu" => "arm64_linux",
        _ => "unsupported",
    }
}

fn all_suffixes() -> impl IntoIterator<Item = &'static str> {
    [
        ".arm64_linux.tar.gz",
        ".arm64_sonoma.tar.gz",
        "x86_64_linux.tar.gz",
        // We follow the Homebrew convention that if there's no arch, it defaults to x86.
        ".ventura.tar.gz",
    ]
}

/// Creates a Rubies info struct from a release asset
fn ruby_from_asset(asset: &Asset) -> Result<Ruby> {
    let version: rv_ruby::version::RubyVersion = {
        let mut curr = asset.name.as_str();
        for suffix in all_suffixes() {
            curr = curr.strip_suffix(suffix).unwrap_or(curr);
        }
        curr.parse()
    }?;
    let display_name = version.to_string();

    let arch_str = ARCH_REGEX
        .captures(&asset.name)
        .and_then(|caps| caps.name("arch"))
        .map_or("unknown", |m| m.as_str());

    let (os, arch) = parse_arch_str(arch_str);

    Ok(Ruby {
        key: format!("{display_name}-{os}-{arch}"),
        version,
        path: Utf8PathBuf::from(&asset.browser_download_url),
        symlink: None,
        arch: arch.to_string(),
        os: os.to_string(),
        gem_root: None,
    })
}

/// Fetches available rubies
pub(crate) async fn fetch_available_rubies(cache: &rv_cache::Cache) -> Result<Release> {
    let cache_entry = cache.entry(
        rv_cache::CacheBucket::Ruby,
        "releases",
        "available_rubies.json",
    );
    let client = reqwest::Client::new();

    let api_base =
        std::env::var("RV_RELEASES_URL").unwrap_or_else(|_| "https://api.github.com".to_string());
    if api_base == "-" {
        // Special case to return empty list
        tracing::debug!("RV_RELEASES_URL is '-', returning empty list without network request.");
        return Ok(Release {
            name: "Empty release".to_owned(),
            assets: Vec::new(),
        });
    }
    let url = format!("{}/repos/spinel-coop/rv-ruby/releases/latest", api_base);

    // 1. Try to read from the disk cache.
    let cached_data: Option<CachedRelease> =
        if let Ok(content) = fs::read_to_string(cache_entry.path()) {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

    // 2. If we have fresh cached data, use it immediately.
    if let Some(cache) = &cached_data {
        if SystemTime::now() < cache.expires_at {
            debug!("Using cached list of available rubies.");
            return Ok(cache.release.clone());
        }
        debug!("Cached ruby list is stale, re-validating with server.");
    }

    // 3. Cache is stale or missing
    let etag = cached_data.as_ref().and_then(|c| c.etag.clone());
    let mut request_builder = client
        .get(url)
        .header("User-Agent", "rv-cli")
        .header("Accept", "application/vnd.github+json");

    // 4. Use ETag for conditional requests if we have one
    if let Some(etag) = &etag {
        debug!("Using ETag to make a conditional request: {}", etag);
        request_builder = request_builder.header("If-None-Match", etag.clone());
    }

    let response = request_builder.send().await?;

    // 4. Handle the server's response.
    match response.status() {
        reqwest::StatusCode::NOT_MODIFIED => {
            debug!("GitHub API confirmed releases list is unchanged (304 Not Modified).");
            let mut stale_cache =
                cached_data.ok_or_else(|| io::Error::other("304 response without prior cache"))?;

            // Update the expiry time based on the latest Cache-Control header
            let max_age = response
                .headers()
                .get("Cache-Control")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_max_age)
                .unwrap_or(Duration::from_secs(60));

            stale_cache.expires_at = SystemTime::now() + max_age.max(MINIMUM_CACHE_TTL);
            fs::write(cache_entry.path(), serde_json::to_string(&stale_cache)?)?;
            Ok(stale_cache.release)
        }
        reqwest::StatusCode::OK => {
            debug!("Received new releases list from GitHub (200 OK).");
            let headers = response.headers().clone();
            let new_etag = headers
                .get("ETag")
                .and_then(|v| v.to_str().ok())
                .map(String::from);

            let max_age = headers
                .get("Cache-Control")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_max_age)
                .unwrap_or(Duration::from_secs(60)); // Default to 60s if header is missing

            let release: Release = response.json().await?;
            debug!("Fetched latest release {}", release.name);

            let new_cache_entry = CachedRelease {
                expires_at: SystemTime::now() + max_age.max(MINIMUM_CACHE_TTL),
                etag: new_etag,
                release: release.clone(),
            };

            if let Some(parent) = cache_entry.path().parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(cache_entry.path(), serde_json::to_string(&new_cache_entry)?)?;

            Ok(release)
        }
        status => {
            warn!("Failed to fetch releases, status: {}", status);
            Err(response.error_for_status().unwrap_err().into())
        }
    }
}

/// Lists the available and installed rubies.
pub async fn list(config: &Config, format: OutputFormat, installed_only: bool) -> Result<()> {
    let installed_rubies = config.rubies();
    let active_ruby = config.project_ruby();

    if installed_only {
        if installed_rubies.is_empty() && format == OutputFormat::Text {
            warn!("No Ruby installations found.");
            info!("Try installing Ruby with 'rv ruby install <version>'");
            return Ok(());
        }

        let entries: Vec<JsonRubyEntry> = installed_rubies
            .into_iter()
            .map(|ruby| {
                let active = active_ruby.as_ref().is_some_and(|a| a == &ruby);
                JsonRubyEntry {
                    installed: true,
                    active,
                    details: ruby,
                }
            })
            .collect();

        return print_entries(&entries, format);
    }

    let release = match fetch_available_rubies(&config.cache).await {
        Ok(release) => release,
        Err(e) => {
            warn!(
                "Could not fetch or re-validate available Ruby versions: {}",
                e
            );
            let cache_entry = config.cache.entry(
                rv_cache::CacheBucket::Ruby,
                "releases",
                "available_rubies.json",
            );
            if let Ok(content) = fs::read_to_string(cache_entry.path())
                && let Ok(cached_data) = serde_json::from_str::<CachedRelease>(&content)
            {
                warn!("Displaying stale list of available rubies from cache.");
                cached_data.release
            } else {
                Release {
                    name: "Empty".to_owned(),
                    assets: Vec::new(),
                }
            }
        }
    };

    // Might have multiple installed rubies with the same version (e.g., "ruby-3.2.0" and "mruby-3.2.0").
    let mut rubies_map: BTreeMap<String, Vec<Ruby>> = BTreeMap::new();
    for ruby in installed_rubies {
        rubies_map
            .entry(ruby.display_name())
            .or_default()
            .push(ruby);
    }

    // Filter releases+assets for current platform
    let (desired_os, desired_arch) = parse_arch_str(current_platform_arch_str());
    let rubies_for_this_platform: Vec<Ruby> = release
        .assets
        .iter()
        .filter_map(|asset| ruby_from_asset(asset).ok())
        .filter(|ruby| ruby.os == desired_os && ruby.arch == desired_arch)
        .collect();

    let available_rubies = latest_patch_version(rubies_for_this_platform);

    debug!(
        "Found {} available rubies for platform {}/{}",
        available_rubies.len(),
        desired_os,
        desired_arch
    );

    // Merge in installed rubies, replacing any available ones with the installed versions
    for ruby in available_rubies {
        if !rubies_map.contains_key(&ruby.display_name()) {
            rubies_map
                .entry(ruby.display_name())
                .or_default()
                .push(ruby);
        }
    }

    if rubies_map.is_empty() && format == OutputFormat::Text {
        warn!("No rubies found for your platform.");
        return Ok(());
    }

    // Create entries for output
    let entries: Vec<JsonRubyEntry> = rubies_map
        .into_values()
        .flatten()
        .map(|ruby| {
            let installed = !ruby.path.as_str().starts_with("http");
            let active = active_ruby.as_ref().is_some_and(|a| a == &ruby);
            JsonRubyEntry {
                installed,
                active,
                details: ruby,
            }
        })
        .collect();

    print_entries(&entries, format)
}

fn latest_patch_version(rubies_for_this_platform: Vec<Ruby>) -> Vec<Ruby> {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct NonPatchRelease {
        engine: rv_ruby::engine::RubyEngine,
        major: Option<rv_ruby::request::VersionPart>,
        minor: Option<rv_ruby::request::VersionPart>,
    }

    impl From<RubyRequest> for NonPatchRelease {
        fn from(value: RubyRequest) -> Self {
            Self {
                engine: value.engine,
                major: value.major,
                minor: value.minor,
            }
        }
    }
    let mut available_rubies: BTreeMap<NonPatchRelease, Ruby> = BTreeMap::new();
    for ruby in rubies_for_this_platform {
        let key = NonPatchRelease::from(ruby.version.clone());
        let skip = available_rubies
            .get(&key)
            .map(|other| other.version > ruby.version)
            .unwrap_or_default();
        if !skip {
            available_rubies.insert(key, ruby);
        }
    }
    available_rubies.into_values().collect()
}

fn print_entries(entries: &[JsonRubyEntry], format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            let width = entries
                .iter()
                .map(|e| e.details.display_name().len())
                .max()
                .unwrap_or(0);
            for entry in entries {
                println!("{}", format_ruby_entry(entry, width));
            }
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(io::stdout(), entries)?;
        }
    }
    Ok(())
}

/// Formats a single entry for text output.
fn format_ruby_entry(entry: &JsonRubyEntry, width: usize) -> String {
    let marker = if entry.active { "*" } else { " " };
    let name = entry.details.display_name();

    if entry.installed {
        format!(
            "{marker} {name:width$} {} {}",
            "[installed]".green(),
            entry.details.executable_path().cyan()
        )
    } else {
        format!("{marker} {name:width$} {}", "[available]".dimmed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use rv_ruby::version::RubyVersion;
    use std::str::FromStr as _;

    #[test]
    fn test_parse_cache_header() {
        let input_header = "Cache-Control: max-age=3600, must-revalidate";
        let actual = parse_max_age(input_header).unwrap();
        let expected = Duration::from_secs(3600);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_deser_release() {
        let jtxt = std::fs::read_to_string("../../api.json").unwrap();
        let release: Release = serde_json::from_str(&jtxt).unwrap();
        dbg!(&release);
    }

    #[test]
    fn test_latest_patch_version() {
        struct Test {
            name: &'static str,
            input: Vec<Ruby>,
            expected: Vec<Ruby>,
        }

        fn ruby(version: &str) -> Ruby {
            let version = RubyVersion::from_str(version).unwrap();
            let version_str = version.to_string();
            Ruby {
                key: format!("{version_str}-macos-aarch64"),
                version,
                path: Utf8PathBuf::from(format!("/tmp/{version_str}")),
                symlink: None,
                arch: "aarch64".into(),
                os: "macos".into(),
                gem_root: None,
            }
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
                name: "prefers_latest_prerelease_when_no_final",
                input: vec![
                    ruby("ruby-3.2.0-preview1"),
                    ruby("ruby-3.2.0-rc1"),
                    ruby("ruby-3.2.0-preview3"),
                ],
                expected: vec![ruby("ruby-3.2.0-rc1")],
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
            let actual = latest_patch_version(input);
            assert_eq!(
                actual, expected,
                "Failed test {name}, got {actual:?} but expected {expected:?}"
            );
        }
    }
}
