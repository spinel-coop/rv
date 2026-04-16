use std::fs;
use std::sync::Arc;

use owo_colors::OwoColorize;
use rv_client::registry_client::RegistryClient;
use rv_gem_types::ReleaseTuple;
use rv_lockfile::datatypes::GemfileDotLock;
use rv_version::Version;
use tracing::debug;

use crate::{
    GlobalArgs,
    commands::{clean_install::InstallStats, tool::Installed},
    config::Config,
    gemserver::{self, GemName, GemRelease, Gemserver},
    resolver::GemDepsMap,
};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error(transparent)]
    RegistryClientError(#[from] rv_client::registry_client::Error),
    #[error("No version {0} available")]
    NoVersionFound(Version),
    #[error("The gem does not actually have any releases published")]
    NoReleasesPublished,
    #[error(transparent)]
    VersionError(#[from] rv_version::VersionError),
    #[error(transparent)]
    GemserverError(#[from] gemserver::Error),
    #[error("Could not parse a version from the server")]
    GemReleaseParse(#[from] gemserver::GemReleaseParse),
    #[error("Could not create the cache dir: {0}")]
    CouldNotCreateCacheDir(std::io::Error),
    #[error("Could not write to the cache: {0}")]
    CouldNotWriteToCache(std::io::Error),
    #[error("Could not choose version: {0}")]
    CouldNotChooseVersion(String),
    #[error(transparent)]
    InstallError(#[from] crate::commands::clean_install::Error),
    #[error("Could not pin Ruby version for this tool: {0}")]
    CouldNotPinRubyVersion(std::io::Error),
    #[error(
        "The gem {0} cannot be installed as a tool because it provides no executable named {0}"
    )]
    NoMatchingExecutable(String),
}

type Result<T> = std::result::Result<T, Error>;

pub(crate) async fn install(
    global_args: &GlobalArgs,
    gem: GemName,
    gem_server: String,
    force: bool,
) -> Result<Installed> {
    let config = &Config::new(global_args, None)?;

    config.self_update_if_needed().await;

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

    let registry_client = Arc::new(RegistryClient::new(&gem_server, "tool install")?);
    let gemserver = Gemserver::new(config, registry_client.clone())?;

    // Look up the gem to install.
    let releases = gemserver.get_releases_for_gem(&gem_name).await?;
    debug!("Found {} releases for the gem {}", releases.len(), gem_name);
    if releases.is_empty() {
        return Err(Error::NoReleasesPublished);
    }

    let release_to_install = match gem_version {
        Some(user_choice) => releases
            .iter()
            .filter(|gem_release| gem_release.version() == &user_choice)
            .max_by(|x, y| x.version_platform().cmp(y.version_platform()))
            .map_or_else(
                || Err(Error::NoVersionFound(user_choice)),
                |v| Ok(v.to_owned()),
            )?,
        _ => releases
            .iter()
            .max_by(|x, y| x.version_platform().cmp(y.version_platform()))
            .map_or_else(|| Err(Error::NoReleasesPublished), |v| Ok(v.to_owned()))?,
    };

    debug!("Selected {} {}", gem_name, release_to_install.full_name());

    let target_version = release_to_install.version_platform();

    let mut gems_to_deps = GemDepsMap::default();

    gems_to_deps.insert(
        gem_name.clone(),
        [(target_version.clone(), release_to_install.clone())].into(),
    );

    // Check if the tool was already installed.
    let install_path = super::tool_dir_for(&gem_name, &target_version.to_string());
    let already_installed = install_path.exists();
    if already_installed {
        if force {
            debug!("Reinstalling tool");
        } else {
            println!(
                "{} {} already installed at {}",
                gem_name.cyan(),
                target_version,
                install_path.cyan(),
            );
            return Ok(Installed {
                version: release_to_install.version().to_owned(),
                dir: install_path,
            });
        }
    }

    let ruby_to_use = config
        .best_ruby_matching_requirement(&release_to_install.metadata.ruby)
        .await?;
    debug!("Selected Ruby {ruby_to_use} for this gem");

    debug!("Querying all transitive dependencies");
    gems_to_deps.extend(
        gemserver
            .query_all_gem_deps(&release_to_install, &ruby_to_use)
            .await?,
    );
    debug!("Retrieved all transitive deps.");

    // OK, now we know all transitive dependencies, and have a dependency graph.
    // Now, translate the dependency constraint list into a PubGrub system, and resolve
    // (i.e. figure out which version of every gem will be used.)
    debug!("Resolving all dependencies via PubGrub");
    let versions_needed =
        crate::resolver::solve(gem_name.clone(), release_to_install.clone(), gems_to_deps)
            .map_err(|e| Error::CouldNotChooseVersion(e.to_string()))?;
    debug!("All dependencies resolved");

    // Make a Gemfile.lock in-memory, install it via `rv ci`.
    let lockfile_builder = LockfileBuilder {
        gemserver_remote: registry_client.url(),
        versions_needed,
    };
    let lockfile = lockfile_builder.lockfile();

    let result = crate::commands::clean_install::install_tool_lockfile(
        global_args,
        Some(ruby_to_use.clone().into()),
        lockfile,
        install_path.clone(),
    )
    .await;

    match result {
        Ok(InstallStats {
            executables_installed,
        }) => {
            if !executables_installed.contains(&gem_name) {
                fs::remove_dir_all(install_path).unwrap();
                return Err(Error::NoMatchingExecutable(gem_name.clone()));
            }
        }
        Err(error) => {
            fs::remove_dir_all(install_path).unwrap();
            return Err(Error::InstallError(error));
        }
    }
    let pin_path = install_path.join(".ruby-version");
    fs::write(&pin_path, format!("{ruby_to_use}\n")).map_err(Error::CouldNotPinRubyVersion)?;
    debug!("Pinned dir {} to {}", pin_path, ruby_to_use);
    println!(
        "Installed {} version {} to {}",
        gem_name.cyan(),
        target_version,
        install_path.cyan(),
    );
    Ok(Installed {
        version: release_to_install.version().to_owned(),
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
    versions_needed: Vec<(ReleaseTuple, GemRelease)>,
    gemserver_remote: String,
}

impl LockfileBuilder {
    /// Create an in-memory Gemfile.lock that views/borrows its data from this builder.
    pub fn lockfile(&self) -> GemfileDotLock<'_> {
        let mut lockfile = rv_lockfile::datatypes::GemfileDotLock::default();
        let mut gem_section = rv_lockfile::datatypes::GemSection {
            remote: Some(&self.gemserver_remote),
            specs: Vec::new(),
        };
        let mut checksums = vec![];
        for (release_tuple, gem_release) in &self.versions_needed {
            let spec = Self::spec_for_gem_dep(release_tuple);
            gem_section.specs.push(spec);
            let checksum = Self::checksum_for_spec(release_tuple, gem_release);
            checksums.push(checksum);
        }

        lockfile.gem.push(gem_section);
        lockfile.checksums = Some(checksums);
        lockfile
    }

    fn spec_for_gem_dep(release_tuple: &ReleaseTuple) -> rv_lockfile::datatypes::Spec {
        rv_lockfile::datatypes::Spec {
            // We don't need to know the deps here, we've already resolved all dependencies.
            // A real Gemfile.lock would populate them, but for this command we don't need to.
            deps: Vec::new(),
            release_tuple: release_tuple.clone(),
        }
    }

    fn checksum_for_spec<'a>(
        release_tuple: &ReleaseTuple,
        gem_release: &GemRelease,
    ) -> rv_lockfile::datatypes::Checksum<'a> {
        rv_lockfile::datatypes::Checksum {
            release_tuple: release_tuple.clone(),
            algorithm: rv_lockfile::datatypes::ChecksumAlgorithm::SHA256,
            value: gem_release.metadata.checksum.clone(),
        }
    }
}
