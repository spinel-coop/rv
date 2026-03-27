use futures_util::{StreamExt, stream::FuturesUnordered};
use rv_ruby::version::RubyVersion;
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Mutex,
};
use tracing::debug;

use super::Result;
use crate::gemserver::{self, GemRelease, Gemserver};

async fn fetch(
    req: String,
    gemserver: &Gemserver,
) -> Result<((String, Vec<GemRelease>), Vec<String>)> {
    debug!("Fetching {req}");
    let dep_info_resp = gemserver.get_releases_for_gem(&req).await?;
    let dep_versions = gemserver::parse_release_from_body(&dep_info_resp)?;
    let transitive_deps = dep_versions
        .iter()
        .flat_map(|d| d.clone().deps.into_iter().map(|d| d.name))
        .collect();
    Ok(((req, dep_versions), transitive_deps))
}

pub async fn query_all_gem_deps(
    root: GemRelease,
    gemserver: &Gemserver,
    gems_to_deps: &mut HashMap<String, Vec<GemRelease>>,
    ruby_to_use: &RubyVersion,
) -> Result<()> {
    let results = Rc::new(Mutex::new(HashMap::<String, Vec<GemRelease>>::new()));
    let mut in_flight = FuturesUnordered::new();
    let seen_requests = Rc::new(Mutex::new(HashSet::<String>::new()));

    // Initial requests
    for d in root.deps {
        let req = d.name;
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
                    version
                        .metadata
                        .ruby
                        .satisfied_by(&rv_version::Version::from(ruby_to_use))
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
