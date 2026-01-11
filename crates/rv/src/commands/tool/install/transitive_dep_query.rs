use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Mutex,
};
use tracing::debug;

use super::Result;
use crate::commands::tool::install::gemserver::{self, Gemserver, VersionAvailable};

type Request = String; // Gem name
type Item = (String, Vec<VersionAvailable>); // (name, deps) pair.

async fn fetch(req: Request, gemserver: &Gemserver) -> Result<(Item, Vec<Request>)> {
    debug!("Fetching {req}");
    let dep_info_resp = gemserver.get_versions_for_gem(&req).await?;
    let dep_versions = gemserver::parse_version_from_body(&dep_info_resp)?;
    let transitive_deps = dep_versions
        .iter()
        .flat_map(|d| d.clone().deps.into_iter().map(|d| d.gem_name))
        .collect();
    Ok(((req, dep_versions), transitive_deps))
}

pub async fn query_all_gem_deps_from_server(
    root: VersionAvailable,
    gemserver: &Gemserver,
    gems_to_deps: &mut HashMap<String, Vec<VersionAvailable>>,
) -> Result<()> {
    let results = Rc::new(Mutex::new(HashMap::<String, Vec<VersionAvailable>>::new()));
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
            let mut map = results.lock().expect("Lock poisoned");
            map.insert(dep_name, dep_info);
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
