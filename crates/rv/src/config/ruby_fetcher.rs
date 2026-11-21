use std::{
    io,
    time::{Duration, SystemTime},
};

use super::Config;
use fs_err as fs;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use rv_platform::HostPlatform;
use rv_ruby::{
    Index, IndexEntry, RemoteRuby,
    engine::RubyEngine,
    request::RequestError,
    version::{ParseVersionError, RubyVersion},
};

// Use the server's TTL, but don't re-check more than every 60 seconds.
const MINIMUM_CACHE_TTL: Duration = Duration::from_secs(60);

const INDEX_CACHE_FILE: &str = "index.json";

static PARSE_MAX_AGE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"max-age=(\d+)").unwrap());

// Holds the index along with its ETag and calculated expiry time
#[derive(Serialize, Deserialize, Debug)]
struct CachedIndex {
    expires_at: SystemTime,
    etag: Option<String>,
    index: Index,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Request(#[from] RequestError),
    #[error("Failed to fetch available ruby versions from rv.dev")]
    IndexRequest(#[from] reqwest::Error),
    #[error(transparent)]
    ParseVersion(#[from] ParseVersionError),
}

type Result<T> = miette::Result<T, Error>;

impl Config {
    /// Discover all remotely available Ruby versions with caching.
    ///
    /// Every engine and platform is listed in one index at rv.dev/ruby. This returns
    /// the rubies in it that run on the host.
    pub async fn discover_remote_rubies(&self) -> Vec<RemoteRuby> {
        if self.offline {
            debug!("OFFLINE: skipping remote ruby fetch");
            return vec![];
        }

        let host = match HostPlatform::current() {
            Ok(h) => h,
            Err(e) => {
                warn!("Could not detect host platform: {e}");
                return vec![];
            }
        };

        let url = index_url();
        let index = match fetch_index(&self.cache, &url).await {
            Ok(index) => index,
            Err(e) => {
                warn!("Could not fetch available Ruby versions: {}", e);
                stale_cache_fallback(&self.cache, &url)
            }
        };

        let rubies = rubies_for(&index, &host);

        debug!(
            "Found {} available rubies for platform {}/{}",
            rubies.len(),
            host.os(),
            host.arch()
        );

        rubies
    }

    /// What the index says about one version on this host, going by the copy on disk.
    ///
    /// Never makes a request. An index that is past its expiry is ignored, because the
    /// build it lists may have been replaced since.
    pub fn cached_index_entry(&self, engine: &RubyEngine, version: &str) -> Option<IndexEntry> {
        let host = HostPlatform::current().ok()?;
        let cache_entry = self.cache.entry(
            rv_cache::CacheBucket::Ruby,
            "releases",
            cache_key_for(&index_url()),
        );
        let content = fs::read_to_string(cache_entry.path()).ok()?;
        let cached: CachedIndex = serde_json::from_str(&content).ok()?;
        if SystemTime::now() >= cached.expires_at {
            return None;
        }

        entry_for(&cached.index, engine, version, &host).cloned()
    }
}

fn cache_key_for(url: &str) -> String {
    rv_cache::cache_digest(format!("{}-{}", url, INDEX_CACHE_FILE))
}

fn index_url() -> String {
    std::env::var("RV_LIST_URL").unwrap_or_else(|_| "https://rv.dev/ruby/v1/index.json".to_string())
}

fn empty_index() -> Index {
    Index {
        schema: 1,
        generated_at: String::new(),
        rubies: Vec::new(),
    }
}

/// Fetches the index with ETag/TTL caching.
async fn fetch_index(cache: &rv_cache::Cache, url: &str) -> Result<Index> {
    let client = reqwest::Client::new();

    if url == "-" {
        debug!("RV_LIST_URL is '-', returning empty list without network request.");
        return Ok(empty_index());
    }

    let cache_entry = cache.entry(rv_cache::CacheBucket::Ruby, "releases", cache_key_for(url));

    // 1. Try to read from the disk cache.
    let cached_data: Option<CachedIndex> =
        if let Ok(content) = fs::read_to_string(cache_entry.path()) {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

    // 2. If we have fresh cached data, use it immediately.
    if let Some(cache) = &cached_data {
        if SystemTime::now() < cache.expires_at {
            debug!("Using cached index from {INDEX_CACHE_FILE}.");
            return Ok(cache.index.clone());
        }
        debug!("Cache {INDEX_CACHE_FILE} is stale, re-validating with server.");
    }

    // 3. Cache is stale or missing.
    let etag = cached_data.as_ref().and_then(|c| c.etag.clone());
    let mut request_builder = client
        .get(url)
        .header("User-Agent", format!("rv-{}", env!("CARGO_PKG_VERSION")));

    if let Some(etag) = &etag {
        debug!("Using ETag for conditional request: {}", etag);
        request_builder = request_builder.header("If-None-Match", etag.clone());
    }

    let response = request_builder.send().await?;

    match response.status() {
        reqwest::StatusCode::NOT_MODIFIED => {
            debug!("Server confirmed {INDEX_CACHE_FILE} is unchanged (304).");
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
            Ok(stale_cache.index)
        }
        reqwest::StatusCode::OK => {
            debug!("Received fresh data for {INDEX_CACHE_FILE} (200 OK).");
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
            let index: Index = serde_json::from_slice(&body)?;

            let new_cache_entry = CachedIndex {
                expires_at: SystemTime::now() + max_age.max(MINIMUM_CACHE_TTL),
                etag: new_etag,
                index: index.clone(),
            };

            if let Some(parent) = cache_entry.path().parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(cache_entry.path(), serde_json::to_string(&new_cache_entry)?)?;

            Ok(index)
        }
        status => {
            warn!("Failed to fetch {INDEX_CACHE_FILE} from {url}, status: {status}");
            Err(response.error_for_status().unwrap_err().into())
        }
    }
}

/// Falls back to a stale cache file when a fresh fetch fails.
fn stale_cache_fallback(cache: &rv_cache::Cache, url: &str) -> Index {
    let cache_entry = cache.entry(rv_cache::CacheBucket::Ruby, "releases", cache_key_for(url));
    if let Ok(content) = fs::read_to_string(cache_entry.path())
        && let Ok(cached_data) = serde_json::from_str::<CachedIndex>(&content)
    {
        warn!("Displaying stale list of available rubies from cache.");
        cached_data.index
    } else {
        empty_index()
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

/// The rubies in the index that run on `host`, oldest first.
fn rubies_for(index: &Index, host: &HostPlatform) -> Vec<RemoteRuby> {
    let platform = host.index_str();

    let mut rubies: Vec<RemoteRuby> = index
        .rubies
        .iter()
        .filter(|entry| entry.platforms.iter().any(|p| p == platform))
        .filter_map(|entry| ruby_from_entry(entry, host).ok())
        .collect();
    rubies.sort();
    rubies
}

/// The index's entry for an exact version on `host`. `dev` counts as a version here.
fn entry_for<'a>(
    index: &'a Index,
    engine: &RubyEngine,
    version: &str,
    host: &HostPlatform,
) -> Option<&'a IndexEntry> {
    let platform = host.index_str();

    index.rubies.iter().find(|entry| {
        entry.engine == *engine
            && entry.version == version
            && entry.platforms.iter().any(|p| p == platform)
    })
}

/// Creates a Rubies info struct from an index entry that runs on `host`.
///
/// Fails for an entry without a version number, which is how `dev` is listed.
fn ruby_from_entry(entry: &IndexEntry, host: &HostPlatform) -> Result<RemoteRuby> {
    let version: RubyVersion = format!("{}-{}", entry.engine, entry.version).parse()?;
    let (os, arch) = (host.os(), host.arch());

    Ok(RemoteRuby {
        key: format!("{version}-{os}-{arch}"),
        version,
        arch: arch.to_string(),
        os: os.to_string(),
        build: entry.build.clone(),
        url: entry.url.clone(),
        size: entry.size,
        sha256: entry.sha256.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Index {
        let json = fs_err::read_to_string("../../testdata/index.json").unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn versions_for(host: HostPlatform) -> Vec<String> {
        rubies_for(&index(), &host)
            .iter()
            .map(|ruby| ruby.version.to_string())
            .collect()
    }

    #[test]
    fn test_parse_cache_header() {
        let input_header = "Cache-Control: max-age=3600, must-revalidate";
        let actual = parse_max_age(input_header).unwrap();
        let expected = Duration::from_secs(3600);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_deser_index() {
        insta::assert_debug_snapshot!(index());
    }

    #[test]
    fn test_ruby_from_entry() {
        let index = index();
        let actual = ruby_from_entry(&index.rubies[1], &HostPlatform::LinuxAarch64).unwrap();
        let expected = RemoteRuby {
            key: "ruby-3.4.11-linux-aarch64".to_owned(),
            version: RubyVersion {
                engine: rv_ruby::engine::RubyEngine::Ruby,
                major: 3,
                minor: 4,
                patch: 11,
                patchlevel: None,
                tiny: None,
                prerelease: None,
            },
            arch: "aarch64".to_owned(),
            os: "linux".to_owned(),
            build: "20261014-2".to_owned(),
            url: "https://rv.dev/ruby/3.4.11/20261014-2/ruby-3.4.11.linux-aarch64.tar.gz"
                .to_owned(),
            size: 41209856,
            sha256: "0b9d7e5c3a1f9d7b5e3c1a0f8e6c4b2a9f7d5b3c1e0a8f6d4c2b0e9a7c5d3f1b".to_owned(),
        };
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_rubies_for_lists_what_runs_on_the_host() {
        assert_eq!(
            versions_for(HostPlatform::MacosAarch64),
            ["ruby-3.4.11", "ruby-4.1.0-preview1", "jruby-10.1.2.0"]
        );
        assert_eq!(
            versions_for(HostPlatform::LinuxAarch64),
            ["ruby-3.4.11", "jruby-10.1.2.0"]
        );
        assert_eq!(versions_for(HostPlatform::LinuxX86_64), ["jruby-10.1.2.0"]);
    }

    #[test]
    fn test_rubies_for_windows_come_from_the_same_index() {
        let rubies = rubies_for(&index(), &HostPlatform::WindowsX86_64);
        let urls: Vec<&str> = rubies.iter().map(|ruby| ruby.url.as_str()).collect();

        assert_eq!(
            urls,
            [
                "https://rv.dev/ruby/3.4.11/20261018/rubyinstaller-3.4.11-1-x64.7z",
                "https://rv.dev/ruby/jruby/10.1.2.0/20261015/jruby-bin-10.1.2.0.zip",
            ]
        );
    }

    #[test]
    fn test_universal_archive_is_one_entry_for_every_platform_it_lists() {
        let hosts = [
            HostPlatform::MacosAarch64,
            HostPlatform::MacosX86_64,
            HostPlatform::LinuxX86_64,
            HostPlatform::LinuxAarch64,
        ];
        for host in hosts {
            let rubies = rubies_for(&index(), &host);
            let jruby = rubies.last().unwrap();
            assert_eq!(jruby.version.to_string(), "jruby-10.1.2.0");
            assert_eq!(jruby.os, host.os());
            assert_eq!(jruby.arch, host.arch());
            assert!(jruby.url.ends_with("jruby-bin-10.1.2.0.tar.gz"));
        }
    }

    #[test]
    fn test_entry_for_finds_an_exact_version_for_the_host() {
        let index = index();
        let entry = |engine, version, host| {
            entry_for(&index, &engine, version, &host).map(|entry| entry.filename.as_str())
        };

        assert_eq!(
            entry(RubyEngine::Ruby, "3.4.11", HostPlatform::LinuxAarch64),
            Some("ruby-3.4.11.linux-aarch64.tar.gz")
        );
        assert_eq!(
            entry(RubyEngine::Ruby, "3.4.11", HostPlatform::WindowsX86_64),
            Some("rubyinstaller-3.4.11-1-x64.7z")
        );
        assert_eq!(
            entry(RubyEngine::Ruby, "dev", HostPlatform::MacosAarch64),
            Some("ruby-dev.macos-aarch64.tar.gz")
        );
        assert_eq!(
            entry(RubyEngine::JRuby, "10.1.2.0", HostPlatform::WindowsAarch64),
            Some("jruby-bin-10.1.2.0.zip")
        );
    }

    #[test]
    fn test_entry_for_needs_the_engine_version_and_platform_to_match() {
        let index = index();
        let entry = |engine, version, host| entry_for(&index, &engine, version, &host);

        assert_eq!(
            entry(RubyEngine::Ruby, "3.4.11", HostPlatform::LinuxX86_64),
            None
        );
        assert_eq!(
            entry(RubyEngine::Ruby, "3.4", HostPlatform::MacosAarch64),
            None
        );
        assert_eq!(
            entry(RubyEngine::JRuby, "3.4.11", HostPlatform::MacosAarch64),
            None
        );
        assert_eq!(
            entry(RubyEngine::Ruby, "dev", HostPlatform::WindowsX86_64),
            None
        );
    }

    #[test]
    fn test_dev_has_no_version_and_is_not_listed() {
        let index = index();
        let dev = index.rubies.iter().find(|e| e.version == "dev").unwrap();

        assert!(ruby_from_entry(dev, &HostPlatform::MacosAarch64).is_err());
    }

    #[test]
    fn test_entries_for_an_unknown_platform_are_ignored() {
        let mut index = index();
        index.rubies[0].platforms = vec!["sparc_solaris".to_owned()];

        let versions: Vec<String> = rubies_for(&index, &HostPlatform::MacosAarch64)
            .iter()
            .map(|ruby| ruby.version.to_string())
            .collect();
        assert_eq!(versions, ["ruby-4.1.0-preview1", "jruby-10.1.2.0"]);
    }
}
