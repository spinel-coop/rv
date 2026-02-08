use std::{
    io,
    time::{Duration, SystemTime},
};

use super::Config;
use camino::Utf8PathBuf;
use current_platform::CURRENT_PLATFORM;
use fs_err as fs;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use rv_ruby::{Asset, Release, Ruby, request::RequestError, version::ParseVersionError};

// Use GitHub's TTL, but don't re-check more than every 60 seconds.
const MINIMUM_CACHE_TTL: Duration = Duration::from_secs(60);

static ARCH_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"ruby-[\d\.a-z-]+\.(?P<arch>[a-zA-Z0-9_]+)\.(?:tar\.gz|7z)").unwrap()
});

static PARSE_MAX_AGE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"max-age=(\d+)").unwrap());

// Updated struct to hold ETag and calculated expiry time
#[derive(Serialize, Deserialize, Debug)]
struct CachedRelease {
    expires_at: SystemTime,
    etag: Option<String>,
    release: Release,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Request(#[from] RequestError),
    #[error("Failed to fetch available ruby versions from GitHub")]
    GithubRequest(#[from] reqwest::Error),
    #[error(transparent)]
    ParseVersion(#[from] ParseVersionError),
}

type Result<T> = miette::Result<T, Error>;

impl Config {
    /// Discover all remotely available Ruby versions with caching
    pub async fn discover_remote_rubies(&self) -> Vec<Ruby> {
        let release = match fetch_available_rubies(&self.cache).await {
            Ok(release) => release,
            Err(e) => {
                warn!(
                    "Could not fetch or re-validate available Ruby versions: {}",
                    e
                );
                let cache_entry = self.cache.entry(
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

        // Filter releases+assets for current platform
        let (desired_os, desired_arch) = current_os_and_arch();

        let mut rubies: Vec<Ruby> = release
            .assets
            .iter()
            .filter_map(|asset| ruby_from_asset(asset).ok())
            .filter(|ruby| ruby.os == desired_os && ruby.arch == desired_arch)
            .collect();
        rubies.sort();

        debug!(
            "Found {} available rubies for platform {}/{}",
            rubies.len(),
            desired_os,
            desired_arch
        );

        rubies
    }
}

/// Fetches available rubies
async fn fetch_available_rubies(cache: &rv_cache::Cache) -> Result<Release> {
    let cache_entry = cache.entry(
        rv_cache::CacheBucket::Ruby,
        "releases",
        "available_rubies.json",
    );
    let client = reqwest::Client::new();

    let url = std::env::var("RV_LIST_URL").unwrap_or_else(|_| {
        "https://api.github.com/repos/spinel-coop/rv-ruby/releases/latest".to_string()
    });
    if url == "-" {
        // Special case to return empty list
        debug!("RV_LIST_URL is '-', returning empty list without network request.");
        return Ok(Release {
            name: "Empty release".to_owned(),
            assets: Vec::new(),
        });
    }

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
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", super::github::GITHUB_API_VERSION);

    // Add GitHub token authentication if available
    // Check GITHUB_TOKEN first (GitHub Actions), then GH_TOKEN (GitHub CLI/general use)
    if let Some(token) = super::github::github_token() {
        debug!("Using authenticated GitHub API request");
        request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
    } else {
        debug!("No GitHub token found, using unauthenticated API request");
    }

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
        "ventura" => ("macos", "x86_64"),
        "sequoia" => ("macos", "x86_64"),
        "x86_64_linux" => ("linux", "x86_64"),
        "arm64_linux" => ("linux", "aarch64"),
        "x64" => ("windows", "x86_64"),
        _ => ("unknown", "unknown"),
    }
}

fn current_os_and_arch() -> (&'static str, &'static str) {
    let platform =
        std::env::var("RV_TEST_PLATFORM").unwrap_or_else(|_| CURRENT_PLATFORM.to_string());

    match platform.as_str() {
        "aarch64-apple-darwin" => ("macos", "aarch64"),
        "x86_64-apple-darwin" => ("macos", "x86_64"),
        "x86_64-unknown-linux-gnu" => ("linux", "x86_64"),
        "aarch64-unknown-linux-gnu" => ("linux", "aarch64"),
        "x86_64-pc-windows-msvc" => ("windows", "x86_64"),
        _ => ("unknown", "unknown"),
    }
}

fn all_suffixes() -> impl IntoIterator<Item = &'static str> {
    [
        ".arm64_linux.tar.gz",
        ".arm64_sonoma.tar.gz",
        ".x86_64_linux.tar.gz",
        // We follow the Homebrew convention that if there's no arch, it defaults to x86.
        ".ventura.tar.gz",
        ".sequoia.tar.gz",
        ".x64.7z",
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
        managed: false,
        symlink: None,
        arch: arch.to_string(),
        os: os.to_string(),
        gem_root: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_ruby::version::RubyVersion;

    #[test]
    fn test_parse_cache_header() {
        let input_header = "Cache-Control: max-age=3600, must-revalidate";
        let actual = parse_max_age(input_header).unwrap();
        let expected = Duration::from_secs(3600);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_deser_release() {
        let jtxt = fs_err::read_to_string("../../testdata/api.json").unwrap();
        let release: Release = serde_json::from_str(&jtxt).unwrap();
        let actual = ruby_from_asset(&release.assets[0]).unwrap();
        let expected = Ruby {
            key: "ruby-3.3.0-linux-aarch64".to_owned(),
            version: RubyVersion {
                engine: rv_ruby::engine::RubyEngine::Ruby,
                major: 3,
                minor: 3,
                patch: 0,
                tiny: None,
                prerelease: None,
            },
            path: "https://github.com/spinel-coop/rv-ruby/releases/download/20251006/ruby-3.3.0.arm64_linux.tar.gz".into(),
            managed: false,
            symlink: None,
            arch: "aarch64".to_owned(),
            os: "linux".to_owned(),
            gem_root: None,
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_current_os_and_arch_windows() {
        // SAFETY: This test is run in a single-threaded context.
        unsafe { std::env::set_var("RV_TEST_PLATFORM", "x86_64-pc-windows-msvc") };
        let (os, arch) = current_os_and_arch();
        unsafe { std::env::remove_var("RV_TEST_PLATFORM") };

        assert_eq!(os, "windows");
        assert_eq!(arch, "x86_64");
    }

    #[test]
    fn test_parse_arch_str_windows() {
        let (os, arch) = parse_arch_str("x64");
        assert_eq!(os, "windows");
        assert_eq!(arch, "x86_64");
    }

    #[test]
    fn test_ruby_from_asset_windows() {
        let asset = Asset {
            name: "ruby-3.3.0.x64.7z".to_owned(),
            browser_download_url: "https://example.com/ruby-3.3.0.x64.7z".to_owned(),
        };
        let ruby = ruby_from_asset(&asset).unwrap();
        assert_eq!(ruby.os, "windows");
        assert_eq!(ruby.arch, "x86_64");
        assert_eq!(ruby.version.major, 3);
        assert_eq!(ruby.version.minor, 3);
        assert_eq!(ruby.version.patch, 0);
    }
}
