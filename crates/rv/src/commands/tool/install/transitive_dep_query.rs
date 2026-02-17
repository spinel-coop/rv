use futures_util::{StreamExt, stream::FuturesUnordered};
use rv_ruby::version::ReleasedRubyVersion;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Mutex,
    time::{Duration, SystemTime},
};
use tracing::debug;

use super::Error;
use super::Result;
use crate::{
    commands::tool::install::gemserver::{self, GemRelease, Gemserver},
    config::Config,
};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CachedGemDeps {
    expires_at: SystemTime,
    pub gems_to_deps: HashMap<String, Vec<GemRelease>>,
}

pub(crate) async fn query_all_gem_deps(
    config: &Config,
    gems_to_deps: &mut HashMap<String, Vec<GemRelease>>,
    root: GemRelease,
    root_gem_name: &str,
    gemserver: &Gemserver,
    ruby_to_use: &ReleasedRubyVersion,
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
            root.version(),
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
    query_all_gem_deps_from_server(root, gemserver, gems_to_deps, ruby_to_use).await?;

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

type Request = String; // Gem name
type Item = (String, Vec<GemRelease>); // (name, deps) pair.

async fn fetch(req: Request, gemserver: &Gemserver) -> Result<(Item, Vec<Request>)> {
    debug!("Fetching {req}");
    let dep_info_resp = gemserver.get_releases_for_gem(&req).await?;
    let dep_versions = gemserver::parse_release_from_body(&dep_info_resp)?;
    let transitive_deps = dep_versions
        .iter()
        .flat_map(|d| d.clone().deps.into_iter().map(|d| d.gem_name))
        .collect();
    Ok(((req, dep_versions), transitive_deps))
}

pub async fn query_all_gem_deps_from_server(
    root: GemRelease,
    gemserver: &Gemserver,
    gems_to_deps: &mut HashMap<String, Vec<GemRelease>>,
    ruby_to_use: &ReleasedRubyVersion,
) -> Result<()> {
    let results = Rc::new(Mutex::new(HashMap::<String, Vec<GemRelease>>::new()));
    let mut in_flight = FuturesUnordered::new();
    let seen_requests = Rc::new(Mutex::new(HashSet::<Request>::new()));

    // Initial requests
    for d in root.deps.clone() {
        let req = d.gem_name;
        debug!("Queuing {req}");
        in_flight.push(fetch(req, gemserver))
    }

    // Keep fetching new dependencies we discover.
    while let Some(res) = in_flight.next().await {
        let ((dep_name, dep_info), new_deps) = res?;
        {
            let mut results = results.lock().expect("Lock poisoned");
            // Skip possible versions that are incompatible with our
            // chosen Ruby version.
            // We should filter these out now, so that we minimize the number
            // of deps that PubGrub has to consider.
            let candidate_versions = dep_info
                .into_iter()
                .filter(|version| {
                    super::choosing_ruby_version::does_ruby_version_satisfy(
                        &ruby_to_use.clone(),
                        &version.metadata.ruby,
                    )
                })
                .collect();
            results.insert(dep_name, candidate_versions);
        }

        for req in new_deps {
            if seen_requests
                .lock()
                .expect("Lock poisoned")
                .insert(req.clone())
            {
                debug!("Queuing {req}");
                in_flight.push(fetch(req, gemserver));
            }
        }
    }

    *gems_to_deps = Rc::into_inner(results).unwrap().into_inner().unwrap();
    Ok(())
}
