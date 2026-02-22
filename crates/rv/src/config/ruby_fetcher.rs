use std::{
    io,
    time::{Duration, SystemTime},
};

use super::Config;
use camino::Utf8PathBuf;
use fs_err as fs;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use rv_platform::HostPlatform;
use rv_ruby::{Asset, Release, Ruby, request::RequestError, version::ParseVersionError};

// Use GitHub's TTL, but don't re-check more than every 60 seconds.
const MINIMUM_CACHE_TTL: Duration = Duration::from_secs(60);

static ARCH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"ruby-[\d\.a-z-]+\.(?P<arch>[a-zA-Z0-9_]+)\.(?:tar\.gz|7z)").unwrap());

static PARSE_MAX_AGE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"max-age=(\d+)").unwrap());

/// Regex for parsing RubyInstaller2 asset filenames.
///
/// Captures: group 1 = Ruby version, group 2 = revision number, group 3 = architecture.
/// Example: `rubyinstaller-3.4.8-1-x64.7z` → version=`3.4.8`, revision=`1`, arch=`x64`.
static RUBYINSTALLER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^rubyinstaller-(.+)-(\d+)-(x64|x86|arm)\.7z$").unwrap());

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
    /// Discover all remotely available Ruby versions with caching.
    ///
    /// On Windows, fetches from `oneclick/rubyinstaller2` (one release per Ruby version).
    /// On other platforms, fetches from `spinel-coop/rv-ruby` (all versions in one release).
    pub async fn discover_remote_rubies(&self) -> Vec<Ruby> {
        // Detect host first — this decides which release source to query.
        let host = match HostPlatform::current() {
            Ok(h) => h,
            Err(e) => {
                warn!("Could not detect host platform: {e}");
                return vec![];
            }
        };

        let ((fetch_result, url), cache_file) = if host.is_windows() {
            (
                fetch_rubyinstaller2_rubies(&self.cache).await,
                "rubyinstaller2.json",
            )
        } else {
            (
                fetch_available_rubies(&self.cache).await,
                "available_rubies.json",
            )
        };

        let release = match fetch_result {
            Ok(release) => release,
            Err(e) => {
                warn!("Could not fetch available Ruby versions: {}", e);
                stale_cache_fallback(&self.cache, cache_file, &url)
            }
        };

        let desired_os = host.os();
        let desired_arch = host.arch();

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

fn cache_key_for(url: &str, cache_file: &str) -> String {
    rv_cache::cache_digest(format!("{}-{}", url, cache_file))
}

fn url_for(env_var: &str, default_url: &str) -> String {
    std::env::var(env_var).unwrap_or_else(|_| default_url.to_string())
}

/// Fetches a GitHub releases endpoint with ETag/TTL caching.
///
/// The `transform` closure converts the raw JSON response body into a `Release`.
/// For rv-ruby this is identity (response is already a single `Release`).
/// For RubyInstaller2 this combines a `Vec<Release>` into one synthetic `Release`.
async fn fetch_cached_github_release(
    cache: &rv_cache::Cache,
    cache_file: &str,
    env_var: &str,
    url: &str,
    transform: impl FnOnce(bytes::Bytes) -> Result<Release>,
) -> Result<Release> {
    let client = reqwest::Client::new();

    if url == "-" {
        debug!("{env_var} is '-', returning empty list without network request.");
        return Ok(Release {
            name: "Empty release".to_owned(),
            assets: Vec::new(),
        });
    }

    let cache_key = cache_key_for(url, cache_file);
    let cache_entry = cache.entry(rv_cache::CacheBucket::Ruby, "releases", cache_key);

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
            debug!("Using cached release data from {cache_file}.");
            return Ok(cache.release.clone());
        }
        debug!("Cache {cache_file} is stale, re-validating with server.");
    }

    // 3. Cache is stale or missing.
    let etag = cached_data.as_ref().and_then(|c| c.etag.clone());
    let mut request_builder = super::github::github_api_get(&client, url);

    if let Some(etag) = &etag {
        debug!("Using ETag for conditional request: {}", etag);
        request_builder = request_builder.header("If-None-Match", etag.clone());
    }

    let response = request_builder.send().await?;

    match response.status() {
        reqwest::StatusCode::NOT_MODIFIED => {
            debug!("Server confirmed {cache_file} is unchanged (304).");
            let mut stale_cache =
                cached_data.ok_or_else(|| io::Error::other("304 response without prior cache"))?;

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
            debug!("Received fresh data for {cache_file} (200 OK).");
            let headers = response.headers().clone();
            let new_etag = headers
                .get("ETag")
                .and_then(|v| v.to_str().ok())
                .map(String::from);

            let max_age = headers
                .get("Cache-Control")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_max_age)
                .unwrap_or(Duration::from_secs(60));

            let body = response.bytes().await?;
            let release = transform(body)?;

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
            warn!("Failed to fetch {cache_file} from {url}, status: {status}");
            Err(response.error_for_status().unwrap_err().into())
        }
    }
}

/// Fetches available rubies from rv-ruby (macOS/Linux).
async fn fetch_available_rubies(cache: &rv_cache::Cache) -> (Result<Release>, String) {
    let env_var = "RV_LIST_URL";
    let default_url = "https://api.github.com/repos/spinel-coop/rv-ruby/releases/latest";
    let url = url_for(env_var, default_url);
    let release =
        fetch_cached_github_release(cache, "available_rubies.json", env_var, &url, |body| {
            Ok(serde_json::from_slice(&body)?)
        })
        .await;
    (release, url)
}

/// Fetches available rubies from RubyInstaller2 (Windows).
async fn fetch_rubyinstaller2_rubies(cache: &rv_cache::Cache) -> (Result<Release>, String) {
    let env_var = "RV_WINDOWS_LIST_URL";
    let default_url = "https://api.github.com/repos/oneclick/rubyinstaller2/releases?per_page=100";
    let url = url_for(env_var, default_url);
    let release =
        fetch_cached_github_release(cache, "rubyinstaller2.json", env_var, &url, |body| {
            let releases: Vec<Release> = serde_json::from_slice(&body)?;
            Ok(combine_rubyinstaller2_releases(releases))
        })
        .await;
    (release, url)
}

/// Falls back to a stale cache file when a fresh fetch fails.
fn stale_cache_fallback(cache: &rv_cache::Cache, cache_file: &str, url: &str) -> Release {
    let cache_key = cache_key_for(url, cache_file);
    let cache_entry = cache.entry(rv_cache::CacheBucket::Ruby, "releases", cache_key);
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

/// Normalizes RubyInstaller2's multi-release format into a single synthetic Release.
///
/// RubyInstaller2 has one GitHub release per Ruby version, each with assets like
/// `rubyinstaller-3.4.8-1-x64.7z`. This function:
/// 1. Filters to `x64` assets only (32-bit and ARM not yet supported)
/// 2. Normalizes names: `rubyinstaller-3.4.8-1-x64.7z` → `ruby-3.4.8.x64.7z`
/// 3. Deduplicates by (version, arch), keeping the highest revision number
/// 4. Returns a single Release with all normalized assets
///
/// The normalized names are designed to match the existing `ARCH_REGEX`, so
/// `ruby_from_asset()` and the platform filtering pipeline work unchanged.
fn combine_rubyinstaller2_releases(releases: Vec<Release>) -> Release {
    use std::collections::HashMap;

    // Key: (version, arch), Value: (revision, normalized Asset)
    let mut best: HashMap<(String, String), (u32, Asset)> = HashMap::new();

    for release in &releases {
        for asset in &release.assets {
            // Skip devkit installers (e.g., "rubyinstaller-devkit-3.4.4-1-x64.exe").
            // The regex's `.7z$` anchor already excludes `.exe` files, but this
            // guards against any future devkit `.7z` assets.
            if asset.name.starts_with("rubyinstaller-devkit-") {
                continue;
            }

            if let Some(caps) = RUBYINSTALLER_REGEX.captures(&asset.name) {
                let version = &caps[1];
                let revision: u32 = caps[2].parse().unwrap_or(0);
                let arch = &caps[3];

                // Only x64 is supported (WindowsX86_64). Skip x86 and arm.
                if arch != "x64" {
                    continue;
                }

                let key = (version.to_string(), arch.to_string());
                let normalized_name = format!("ruby-{version}.{arch}.7z");
                let normalized_asset = Asset {
                    name: normalized_name,
                    browser_download_url: asset.browser_download_url.clone(),
                };

                match best.entry(key) {
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert((revision, normalized_asset));
                    }
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        if revision > e.get().0 {
                            e.insert((revision, normalized_asset));
                        }
                    }
                }
            }
        }
    }

    let mut assets: Vec<Asset> = best.into_values().map(|(_, asset)| asset).collect();
    assets.sort_by(|a, b| a.name.cmp(&b.name));

    Release {
        name: "rubyinstaller2-combined".to_string(),
        assets,
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

/// Creates a Rubies info struct from a release asset
fn ruby_from_asset(asset: &Asset) -> Result<Ruby> {
    let caps = ARCH_REGEX.captures(&asset.name);
    let arch_match = caps.as_ref().and_then(|c| c.name("arch"));
    let arch_str = arch_match.map_or("unknown", |m| m.as_str());

    let (os, arch) = match HostPlatform::from_ruby_arch_str(arch_str) {
        Ok(hp) => (hp.os(), hp.arch()),
        Err(_) => ("unknown", "unknown"),
    };

    // Use the regex match position to slice off the `.arch.ext` suffix,
    // rather than iterating over a list of known suffixes.
    let version_str = match arch_match {
        Some(m) => &asset.name[..m.start() - 1],
        None => asset.name.as_str(),
    };
    let version: rv_ruby::version::ReleasedRubyVersion = version_str.parse()?;
    let display_name = version.to_string();

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
    use rv_ruby::version::ReleasedRubyVersion;

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
            version: ReleasedRubyVersion {
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
    fn test_ruby_from_asset_all_platforms() {
        let cases = [
            ("ruby-3.3.0.arm64_sonoma.tar.gz", "macos", "aarch64"),
            ("ruby-3.3.0.ventura.tar.gz", "macos", "x86_64"),
            ("ruby-3.4.1.sequoia.tar.gz", "macos", "x86_64"),
            ("ruby-3.3.0.x86_64_linux.tar.gz", "linux", "x86_64"),
            ("ruby-3.3.0.arm64_linux.tar.gz", "linux", "aarch64"),
            ("ruby-3.3.0.x64.7z", "windows", "x86_64"),
        ];
        for (filename, expected_os, expected_arch) in cases {
            let asset = Asset {
                name: filename.to_owned(),
                browser_download_url: format!("https://example.com/{filename}"),
            };
            let ruby = ruby_from_asset(&asset).unwrap();
            assert_eq!(ruby.os, expected_os, "Wrong OS for {filename}");
            assert_eq!(ruby.arch, expected_arch, "Wrong arch for {filename}");
            assert_eq!(ruby.version.major, 3, "Wrong major version for {filename}");
        }
    }

    #[test]
    fn test_ruby_from_asset_unknown_arch() {
        let asset = Asset {
            name: "ruby-3.3.0.sparc_solaris.tar.gz".to_owned(),
            browser_download_url: "https://example.com/ruby-3.3.0.sparc_solaris.tar.gz".to_owned(),
        };
        let ruby = ruby_from_asset(&asset).unwrap();
        assert_eq!(ruby.os, "unknown");
        assert_eq!(ruby.arch, "unknown");
        assert_eq!(ruby.version.major, 3);
    }

    fn make_asset(name: &str) -> Asset {
        Asset {
            name: name.to_string(),
            browser_download_url: format!("https://github.com/download/{name}"),
        }
    }

    fn make_release(name: &str, asset_names: &[&str]) -> Release {
        Release {
            name: name.to_string(),
            assets: asset_names.iter().map(|n| make_asset(n)).collect(),
        }
    }

    #[test]
    fn test_combine_rubyinstaller2_releases_basic() {
        let releases = vec![make_release(
            "RubyInstaller-3.4.4-1",
            &[
                "rubyinstaller-3.4.4-1-x64.7z",
                "rubyinstaller-3.4.4-1-x64.exe", // exe → skipped by regex
                "rubyinstaller-3.4.4-1-x64.7z.asc", // .asc → skipped by regex
                "rubyinstaller-3.4.4-1-x86.7z",  // x86 → skipped
                "rubyinstaller-devkit-3.4.4-1-x64.exe", // devkit → skipped
            ],
        )];

        let result = combine_rubyinstaller2_releases(releases);
        assert_eq!(result.assets.len(), 1);
        assert_eq!(result.assets[0].name, "ruby-3.4.4.x64.7z");
        assert!(
            result.assets[0]
                .browser_download_url
                .contains("rubyinstaller-3.4.4-1-x64.7z")
        );
    }

    #[test]
    fn test_combine_rubyinstaller2_releases_dedup_highest_revision() {
        let releases = vec![
            make_release("RubyInstaller-3.3.0-2", &["rubyinstaller-3.3.0-2-x64.7z"]),
            make_release("RubyInstaller-3.3.0-1", &["rubyinstaller-3.3.0-1-x64.7z"]),
        ];

        let result = combine_rubyinstaller2_releases(releases);
        assert_eq!(result.assets.len(), 1);
        // Should keep revision 2, not revision 1
        assert!(
            result.assets[0]
                .browser_download_url
                .contains("rubyinstaller-3.3.0-2-x64.7z")
        );
    }

    #[test]
    fn test_combine_rubyinstaller2_releases_multiple_versions() {
        let releases = vec![
            make_release("RubyInstaller-3.4.4-1", &["rubyinstaller-3.4.4-1-x64.7z"]),
            make_release("RubyInstaller-3.3.7-1", &["rubyinstaller-3.3.7-1-x64.7z"]),
            make_release("RubyInstaller-3.2.8-1", &["rubyinstaller-3.2.8-1-x64.7z"]),
        ];

        let result = combine_rubyinstaller2_releases(releases);
        assert_eq!(result.assets.len(), 3);
        // Assets are sorted by name
        let names: Vec<&str> = result.assets.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "ruby-3.2.8.x64.7z",
                "ruby-3.3.7.x64.7z",
                "ruby-3.4.4.x64.7z"
            ]
        );
    }

    #[test]
    fn test_combine_rubyinstaller2_releases_skips_devkit() {
        let releases = vec![make_release(
            "RubyInstaller-3.4.4-1",
            &[
                "rubyinstaller-devkit-3.4.4-1-x64.7z", // hypothetical devkit .7z
                "rubyinstaller-3.4.4-1-x64.7z",
            ],
        )];

        let result = combine_rubyinstaller2_releases(releases);
        assert_eq!(result.assets.len(), 1);
        assert_eq!(result.assets[0].name, "ruby-3.4.4.x64.7z");
    }

    #[test]
    fn test_normalized_rubyinstaller_asset_matches_arch_regex() {
        // End-to-end: RubyInstaller2 asset → normalized → ARCH_REGEX → ruby_from_asset
        let releases = vec![make_release(
            "RubyInstaller-3.4.8-1",
            &["rubyinstaller-3.4.8-1-x64.7z"],
        )];

        let combined = combine_rubyinstaller2_releases(releases);
        assert_eq!(combined.assets.len(), 1);

        // The normalized asset name should be parseable by our pipeline
        let ruby = ruby_from_asset(&combined.assets[0]).unwrap();
        assert_eq!(ruby.version.major, 3);
        assert_eq!(ruby.version.minor, 4);
        assert_eq!(ruby.version.patch, 8);
        assert_eq!(ruby.os, "windows");
        assert_eq!(ruby.arch, "x86_64");
    }

    #[test]
    fn test_combine_rubyinstaller2_releases_empty() {
        let result = combine_rubyinstaller2_releases(vec![]);
        assert_eq!(result.assets.len(), 0);
        assert_eq!(result.name, "rubyinstaller2-combined");
    }
}
