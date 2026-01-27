use std::{collections::HashMap, fs};

use owo_colors::OwoColorize;
use reqwest::StatusCode;
use rv_lockfile::datatypes::GemfileDotLock;
use rv_ruby::request::Source;
use rv_version::Version as GemVersion;
use tracing::debug;
use url::Url;

use crate::{
    commands::{
        ci::InstallStats,
        tool::{
            Installed,
            install::{
                choosing_ruby_version::ruby_to_use_for,
                gemserver::{Gemserver, VersionAvailable},
            },
        },
    },
    config::Config,
};

mod choosing_ruby_version;
mod gemserver;
mod pubgrub_bridge;
mod transitive_dep_query;

type GemName = String;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("{0} is not a valid URL")]
    BadUrl(String),
    #[error("No gem with that name ({gem_name}) exists on the server {server}")]
    NotFound { gem_name: String, server: String },
    #[error("No version {0} available")]
    NoVersionFound(GemVersion),
    #[error("The gem does not actually have any versions published")]
    NoVersionsPublished,
    #[error(transparent)]
    VersionError(#[from] rv_version::VersionError),
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
    #[error(transparent)]
    InstallError(#[from] crate::commands::ci::Error),
    #[error("rv could not find any Ruby versions to install")]
    NoRubies,
    #[error(
        "No available Ruby matched the Ruby requirements. The requirements were {requirements:?}"
    )]
    NoMatchingRuby {
        requirements: Vec<gemserver::VersionConstraint>,
    },
    #[error("Could not pin Ruby version for this tool: {0}")]
    CouldNotPinRubyVersion(std::io::Error),
    #[error("This gem doesn't have any executables to install")]
    NoExecutables,
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

pub async fn install(
    config: &Config,
    gem: GemName,
    gem_server: String,
    force: bool,
) -> Result<Installed> {
    // Check if 'gem' is in 'gem@version' format.
    // If `gem_version` is None, it means "latest". Otherwise it's a specific version.
    let (gem_name, gem_version) = if let Some((name, gem_version)) = gem.split_once('@') {
        let gem_version = if gem_version == "latest" {
            None
        } else {
            // You don't have to give a version,
            // but if you give one, it has to parse!
            Some(gem_version.parse()?)
        };
        (name.to_owned(), gem_version)
    } else {
        (gem, None)
    };

    let args = InnerArgs::new(gem_name, gem_server)?;

    let gemserver = Gemserver::new(args.gem_server)?;

    // Maps gem names to their dependency lists.
    let mut gems_to_deps: HashMap<GemName, Vec<VersionAvailable>> = HashMap::new();

    // Look up the gem to install.
    let versions_resp = gemserver
        .get_versions_for_gem(&args.gem)
        .await
        .map_err(|e| match e {
            // If the HTTP error was 404, then return a nice error explaining that the gem
            // wasn't found.
            gemserver::Error::Reqwest(e) if e.status() == Some(StatusCode::NOT_FOUND) => {
                Error::NotFound {
                    gem_name: args.gem.to_owned(),
                    server: gemserver.url.to_string(),
                }
            }
            // Otherwise, keep the error as-is.
            other => Error::from(other),
        })?;

    let versions = gemserver::parse_version_from_body(&versions_resp)?;
    debug!("Found {} versions for the gem {}", versions.len(), args.gem);
    if versions.is_empty() {
        return Err(Error::NoVersionsPublished);
    }
    gems_to_deps.insert(args.gem.clone(), versions.clone());

    // Let's install the most recent version.
    let version_to_install = match gem_version {
        Some(user_choice) => {
            let Some(v) = versions
                .iter()
                .find(|version_available| version_available.version == user_choice)
            else {
                return Err(Error::NoVersionFound(user_choice));
            };
            debug!("Selected version {} of gem {}", v.version, args.gem,);
            v.to_owned()
        }
        _ => {
            let Some(v) = versions
                .iter()
                .max_by_key(|version_available| &version_available.version)
            else {
                return Err(Error::NoVersionsPublished);
            };
            debug!("Selected version {} of gem {}", v.version, args.gem,);
            v.to_owned()
        }
    };

    // Check if the tool was already installed.
    let install_path = super::tool_dir_for(&args.gem, &version_to_install.version);
    let already_installed = install_path.exists();
    if already_installed {
        if force {
            debug!("Reinstalling tool");
        } else {
            println!(
                "{} version {} already installed at {}",
                args.gem.cyan(),
                version_to_install.version,
                install_path.cyan(),
            );
            return Ok(Installed {
                version: version_to_install.version,
                dir: install_path,
            });
        }
    }

    let ruby_to_use = ruby_to_use_for(config, &version_to_install.metadata.ruby).await?;
    debug!("Selected Ruby {ruby_to_use} for this gem");

    debug!("Querying all transitive dependencies");
    let mut transitive_deps = Default::default();
    transitive_dep_query::query_all_gem_deps(
        config,
        &mut transitive_deps,
        version_to_install.clone(),
        &args.gem,
        &gemserver,
        &ruby_to_use,
    )
    .await?;
    gems_to_deps.extend(transitive_deps);
    debug!("Retrieved all transitive deps.");

    // OK, now we know all transitive dependencies, and have a dependency graph.
    // Now, translate the dependency constraint list into a PubGrub system, and resolve
    // (i.e. figure out which version of every gem will be used.)
    debug!("Resolving all dependencies via PubGrub");
    let versions_needed = pubgrub_bridge::solve(
        args.gem.clone(),
        version_to_install.version.clone(),
        gems_to_deps,
    )
    .map_err(|e| Error::CouldNotChooseVersion(e.to_string()))?;
    debug!("All dependencies resolved");

    // Make a Gemfile.lock in-memory, install it via `rv ci`.
    let lockfile_builder = LockfileBuilder::new(&gemserver, versions_needed);
    let lockfile = lockfile_builder.lockfile();
    let mut config_for_install = config.clone();
    config_for_install.requested_ruby = Some((ruby_to_use.clone().into(), Source::Other));

    let InstallStats {
        executables_installed,
    } = crate::commands::ci::install_from_lockfile(
        &config_for_install,
        lockfile,
        install_path.clone(),
    )
    .await?;
    if executables_installed == 0 {
        fs::remove_dir_all(install_path).unwrap();
        return Err(Error::NoExecutables);
    }
    let pin_path = install_path.join(".ruby-version");
    fs::write(&pin_path, format!("{ruby_to_use}\n")).map_err(Error::CouldNotPinRubyVersion)?;
    debug!("Pinned dir {} to {}", pin_path, ruby_to_use);
    let gem_name = args.gem.cyan();
    println!(
        "Installed {} version {} to {}",
        gem_name.cyan(),
        version_to_install.version,
        install_path.cyan(),
    );
    Ok(Installed {
        version: version_to_install.version,
        dir: install_path,
    })
}

/// Owns the information needed to create a lockfile.
/// Currently the lockfile has to borrow from something, it does not
/// actually hold any owned data (strings). It just views data
/// from somewhere else (e.g. a file on disk, a network buffer, etc).
///
/// When building a lockfile from a resolved gem list, there's no actual lockfile
/// on disk or anything, so this holds the data (e.g. strings) that the lockfile views.
struct LockfileBuilder {
    versions_needed: Vec<(String, String)>,
    gemserver_remote: String,
}

impl LockfileBuilder {
    pub fn new(
        gemserver: &Gemserver,
        versions_needed: pubgrub::SelectedDependencies<pubgrub_bridge::DepProvider>,
    ) -> Self {
        let versions_needed: Vec<_> = versions_needed
            .into_iter()
            .map(|(gem_name, v)| (gem_name, v.to_string()))
            .collect();
        let gemserver_remote = gemserver.url.to_string();
        Self {
            gemserver_remote,
            versions_needed,
        }
    }

    /// Create an in-memory Gemfile.lock that views/borrows its data from this builder.
    pub fn lockfile(&self) -> GemfileDotLock<'_> {
        let mut lockfile = rv_lockfile::datatypes::GemfileDotLock::default();
        let mut gem_section = rv_lockfile::datatypes::GemSection {
            remote: &self.gemserver_remote,
            specs: Vec::new(),
        };
        for (gem_name, version) in &self.versions_needed {
            let spec = Self::spec_for_gem_dep(gem_name, version);
            gem_section.specs.push(spec);
        }
        lockfile.gem.push(gem_section);
        lockfile
    }

    fn spec_for_gem_dep<'a>(
        gem_name: &'a GemName,
        version: &'a str,
    ) -> rv_lockfile::datatypes::Spec<'a> {
        rv_lockfile::datatypes::Spec {
            // We don't need to know the deps here, we've already resolved all depenendencies.
            // A real Gemfile.lock would populate them, but for this command we don't need to.
            deps: Vec::new(),
            gem_version: rv_lockfile::datatypes::GemVersion {
                name: gem_name,
                version,
            },
        }
    }
}
