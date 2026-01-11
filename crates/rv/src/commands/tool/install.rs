use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use tracing::debug;
use url::Url;

use crate::{
    commands::tool::install::{
        gemserver::{Gemserver, VersionAvailable},
        transitive_dep_query::query_all_gem_deps_from_server,
    },
    config::Config,
};

mod gemserver;
mod transitive_dep_query;

const GEM_COOP: &str = "https://gem.coop/";

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("{0} is not a valid URL")]
    BadUrl(String),
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
    #[error(transparent)]
    GemserverError(#[from] gemserver::Error),
    #[error("Could not parse a version from the server: {0}")]
    VersionAvailableParse(#[from] gemserver::VersionAvailableParse),
    #[error("Could not create the cache dir: {0}")]
    CouldNotCreateCacheDir(std::io::Error),
    #[error("Could not write to the cache: {0}")]
    CouldNotWriteToCache(std::io::Error),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
struct InnerArgs {
    /// Gemserver to install from.
    gem_server: Url,
    /// Gem to install as a tool.
    gem: String,
}

impl InnerArgs {
    fn new(gem: String) -> Result<Self> {
        let out = Self {
            gem_server: GEM_COOP
                .parse()
                .map_err(|_| Error::BadUrl(GEM_COOP.to_owned()))?,
            gem,
        };
        Ok(out)
    }
}

pub async fn install(config: &Config, gem: String) -> Result<()> {
    let args = InnerArgs::new(gem)?;
    let gemserver = Gemserver::new(args.gem_server)?;

    // Maps gem names to their dependency lists.
    let mut gems_to_deps: HashMap<String, Vec<VersionAvailable>> = HashMap::new();

    // Look up the gem to install.
    let versions_resp = gemserver.get_versions_for_gem(&args.gem).await?;
    let versions = gemserver::parse_version_from_body(&versions_resp)?;
    debug!("Found {} versions for the gem {}", versions.len(), args.gem);
    gems_to_deps.insert(args.gem.clone(), versions.clone());

    let most_recent_version = versions
        .iter()
        .max_by_key(|x| &x.version)
        .unwrap()
        .to_owned();
    debug!(
        "Installing gem {} from its most recent version {}",
        args.gem, most_recent_version.version
    );

    query_all_gem_deps(
        config,
        &mut gems_to_deps,
        most_recent_version,
        &args.gem,
        &gemserver,
    )
    .await?;

    // OK, now we know all transitive dependencies, and have a dependency graph.
    // Now, translate the dependency constraint list into a PubGrub system, and resolve
    // (i.e. figure out which version of every gem will be used.)

    // Now, for each gem, download and install the chosen version.
    // I suggest you basically build an in-memory Gemfile.lock and then call `ci::install_from_lockfile`.
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedGemDeps {
    expires_at: SystemTime,
    gems_to_deps: HashMap<String, Vec<VersionAvailable>>,
}

async fn query_all_gem_deps(
    config: &Config,
    gems_to_deps: &mut HashMap<String, Vec<VersionAvailable>>,
    root: VersionAvailable,
    root_gem_name: &str,
    gemserver: &Gemserver,
) -> Result<()> {
    // First, let's check the cache.
    // 0. Initialize the cache.
    let cached_gemspecs_dir = config
        .cache
        .shard(rv_cache::CacheBucket::GemDeps, "gemdeps")
        .into_path_buf();
    fs_err::create_dir_all(&cached_gemspecs_dir).map_err(Error::CouldNotCreateCacheDir)?;

    // 1. Try to read from the disk cache.
    let cache_entry = config.cache.entry(
        rv_cache::CacheBucket::GemDeps,
        "gemdeps",
        format!(
            "{}_{}_{}.json",
            gemserver.url.host_str().unwrap_or_default(),
            root.version,
            root_gem_name
        ),
    );
    let cached_data: Option<CachedGemDeps> =
        if let Ok(content) = fs_err::read_to_string(cache_entry.path()) {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

    // 2. If we have fresh cached data, use it immediately.
    if let Some(cache) = &cached_data {
        if SystemTime::now() < cache.expires_at {
            debug!("Using cached list of transitive dependency versions");
            *gems_to_deps = cache.gems_to_deps.clone();
            return Ok(());
        }
        debug!("Cached ruby list is stale, re-validating with server.");
    }

    // 3. If we couldn't use a cache
    // look up all versions of all transitive dependencies.
    query_all_gem_deps_from_server(root, gemserver, gems_to_deps).await?;

    debug!("Fetched all transitive dependencies");

    let new_cache_entry = CachedGemDeps {
        expires_at: SystemTime::now() + Duration::from_secs(5 * 60),
        gems_to_deps: gems_to_deps.clone(),
    };

    if let Some(parent) = cache_entry.path().parent() {
        fs_err::create_dir_all(parent).map_err(Error::CouldNotCreateCacheDir)?;
    }
    fs_err::write(
        cache_entry.path(),
        serde_json::to_string(&new_cache_entry).expect("serialization should not fail"),
    )
    .map_err(Error::CouldNotWriteToCache)?;
    Ok(())
}
