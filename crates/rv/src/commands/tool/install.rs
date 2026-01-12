use std::collections::HashMap;

use rv_gem_types::Specification;
use tracing::debug;
use url::Url;

use crate::{
    commands::tool::install::{
        gem_version::GemVersion,
        gemserver::{Gemserver, VersionAvailable},
    },
    config::Config,
};

mod gem_version;
mod gemserver;
mod pubgrub_bridge;
mod transitive_dep_query;

type GemName = String;

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
    #[error("Could not choose version: {0}")]
    CouldNotChooseVersion(String),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
struct InnerArgs {
    /// Gemserver to install from.
    gem_server: Url,
    /// Gem to install as a tool.
    gem: GemName,
}

impl InnerArgs {
    fn new(gem: GemName, gem_server: String) -> Result<Self> {
        let out = Self {
            gem_server: gem_server.parse().map_err(|_| Error::BadUrl(gem_server))?,
            gem,
        };
        Ok(out)
    }
}

pub async fn install(config: &Config, gem: GemName, gem_server: String) -> Result<()> {
    let args = InnerArgs::new(gem, gem_server)?;
    let gemserver = Gemserver::new(args.gem_server)?;

    // Maps gem names to their dependency lists.
    let mut gems_to_deps: HashMap<GemName, Vec<VersionAvailable>> = HashMap::new();

    // Look up the gem to install.
    let versions_resp = gemserver.get_versions_for_gem(&args.gem).await?;
    let versions = gemserver::parse_version_from_body(&versions_resp)?;
    debug!("Found {} versions for the gem {}", versions.len(), args.gem);
    gems_to_deps.insert(args.gem.clone(), versions.clone());

    // Let's install the most recent version.
    // TODO: Allow users to choose a specific version via CLI args.
    let version_to_install = versions
        .iter()
        .max_by_key(|x| &x.version)
        .unwrap()
        .to_owned();
    debug!(
        "Selected version {} of gem {}",
        version_to_install.version, args.gem,
    );

    debug!("Querying all transitive dependencies",);
    transitive_dep_query::query_all_gem_deps(
        config,
        &mut gems_to_deps,
        version_to_install.clone(),
        &args.gem,
        &gemserver,
    )
    .await?;
    debug!("Retrieved all transitive deps.",);

    // OK, now we know all transitive dependencies, and have a dependency graph.
    // Now, translate the dependency constraint list into a PubGrub system, and resolve
    // (i.e. figure out which version of every gem will be used.)
    debug!("Resolving all dependencies via PubGrub");
    let versions_needed =
        pubgrub_bridge::solve(args.gem.clone(), version_to_install.version, gems_to_deps)
            .map_err(|e| Error::CouldNotChooseVersion(e.to_string()))?;
    debug!("All dependencies resolved");

    // Now, for each gem, download and install the chosen version.
    // I suggest you basically build an in-memory Gemfile.lock and then call `ci::install_from_lockfile`.
    let mut lockfile = rv_lockfile::datatypes::GemfileDotLock::default();
    let remote = gemserver.url.to_string();
    let mut gem_section = rv_lockfile::datatypes::GemSection {
        remote: &remote,
        specs: Vec::new(),
    };
    for (gem_name, version) in &versions_needed {
        let spec = spec_for_gem_dep(gem_name, version);
        gem_section.specs.push(spec);
    }
    lockfile.gem.push(gem_section);
    // let ci_args = todo!("Instantiate the CI args");
    // crate::commands::ci::install_from_lockfile(config, ci_args, lockfile);
    Ok(())
}

fn spec_for_gem_dep(
    gem_name: &GemName,
    _version: &GemVersion,
) -> rv_lockfile::datatypes::Spec<'static> {
    rv_lockfile::datatypes::Spec {
        // We don't need to know the deps here, we've already resolved all depenendencies.
        // A real Gemfile.lock would populate them, but for this command we don't need to.
        deps: Vec::new(),
        gem_version: rv_lockfile::datatypes::GemVersion {
            name: &gem_name,
            // TODO: The lockfile treats versions as strings,
            // it has to be updated so it parses them into GemVersions too,
            // then we can plug this gemversion into the lockfile.
            version: todo!(),
        },
    }
}
