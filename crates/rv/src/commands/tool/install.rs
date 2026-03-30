use std::fs;

use owo_colors::OwoColorize;
use reqwest::StatusCode;
use rv_gem_types::ReleaseTuple;
use rv_lockfile::datatypes::GemfileDotLock;
use rv_version::Version;
use tracing::debug;
use url::Url;

use crate::{
    GlobalArgs,
    commands::{clean_install::InstallStats, tool::Installed},
    config::Config,
    gemserver::{self, GemName, GemRelease, Gemserver},
    resolver::{ResolutionPackage, ResolutionRoot},
};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error("{0} is not a valid URL")]
    BadUrl(String),
    #[error("{gem_name} doesn't exist on {server}")]
    NotFound { gem_name: String, server: String },
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
    #[error("This gem doesn't have any executables to install")]
    NoExecutables,
}

type Result<T> = std::result::Result<T, Error>;

pub(crate) async fn install(
    global_args: &GlobalArgs,
    gem: GemName,
    gem_server: String,
    force: bool,
) -> Result<Installed> {
    let mut config = Config::new(global_args, None)?;

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

    let gem_server: Url = gem_server.parse().map_err(|_| Error::BadUrl(gem_server))?;

    let mut gemserver = Gemserver::new(&config, gem_server)?;

    // Look up the gem to install.
    let releases_resp = gemserver
        .get_releases_for_gem(&gem_name)
        .await
        .map_err(|e| match e {
            // If the HTTP error was 404, then return a nice error explaining that the gem
            // wasn't found.
            gemserver::Error::Reqwest(e) if e.status() == Some(StatusCode::NOT_FOUND) => {
                Error::NotFound {
                    gem_name: gem_name.to_owned(),
                    server: gemserver.url.to_string(),
                }
            }
            // Otherwise, keep the error as-is.
            other => Error::from(other),
        })?;

    let releases = gemserver::parse_release_from_body(&releases_resp)?;
    debug!("Found {} releases for the gem {}", releases.len(), gem_name);
    if releases.is_empty() {
        return Err(Error::NoReleasesPublished);
    }

    let release_to_install = match gem_version {
        Some(user_choice) => releases
            .into_iter()
            .filter(|gem_release| gem_release.version() == &user_choice)
            .max_by(|x, y| x.version_platform().cmp(y.version_platform()))
            .map_or_else(|| Err(Error::NoVersionFound(user_choice)), Ok)?,
        _ => releases
            .into_iter()
            .max_by(|x, y| x.version_platform().cmp(y.version_platform()))
            .map_or_else(|| Err(Error::NoReleasesPublished), Ok)?,
    };

    debug!("Selected {} {}", gem_name, release_to_install.full_name());

    let target_version = release_to_install.version_platform();

    let root_package = ResolutionPackage::Gem(gem_name.clone());

    gemserver.gems_to_deps.insert(
        root_package.clone(),
        [(target_version.clone(), release_to_install.clone())].into(),
    );

    let root = ResolutionRoot {
        package: root_package,
        version_platform: target_version.clone(),
        deps: release_to_install
            .deps
            .clone()
            .into_iter()
            .collect::<Vec<_>>(),
    };

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

    gemserver.add_transitive_deps(&root, &ruby_to_use).await?;

    // OK, now we know all transitive dependencies, and have a dependency graph.
    // Now, translate the dependency constraint list into a PubGrub system, and resolve
    // (i.e. figure out which version of every gem will be used.)
    debug!("Resolving all dependencies via PubGrub");
    let versions_needed = crate::resolver::solve(&root, &gemserver.gems_to_deps)
        .map_err(|e| Error::CouldNotChooseVersion(e.to_string()))?;
    debug!("All dependencies resolved");

    // Make a Gemfile.lock in-memory, install it via `rv ci`.
    let lockfile_builder = LockfileBuilder {
        gemserver_remote: gemserver.url.to_string(),
        versions_needed,
    };
    let lockfile = lockfile_builder.lockfile();

    config = Config::new(global_args, Some(ruby_to_use.clone().into()))?;

    let result = crate::commands::clean_install::install_inline_lockfile(
        &config,
        lockfile,
        Some(install_path.clone()),
    )
    .await;

    match result {
        Ok(InstallStats {
            executables_installed: 0,
        }) => {
            fs::remove_dir_all(install_path).unwrap();
            return Err(Error::NoExecutables);
        }
        Ok(_) => {}
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
    versions_needed: Vec<(String, GemRelease)>,
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
        for (name, gem_release) in &self.versions_needed {
            let release_tuple = ReleaseTuple {
                name: name.clone(),
                version: gem_release.version().clone(),
                platform: gem_release.platform().clone(),
            };
            let spec = Self::spec_for_gem_dep(release_tuple.clone());
            gem_section.specs.push(spec);
            let checksum = Self::checksum_for_spec(release_tuple, gem_release);
            checksums.push(checksum);
        }

        lockfile.gem.push(gem_section);
        lockfile.checksums = Some(checksums);
        lockfile
    }

    fn spec_for_gem_dep(release_tuple: ReleaseTuple) -> rv_lockfile::datatypes::Spec {
        rv_lockfile::datatypes::Spec {
            // We don't need to know the deps here, we've already resolved all dependencies.
            // A real Gemfile.lock would populate them, but for this command we don't need to.
            deps: Vec::new(),
            release_tuple,
        }
    }

    fn checksum_for_spec<'a>(
        release_tuple: ReleaseTuple,
        gem_release: &GemRelease,
    ) -> rv_lockfile::datatypes::Checksum<'a> {
        rv_lockfile::datatypes::Checksum {
            release_tuple,
            algorithm: rv_lockfile::datatypes::ChecksumAlgorithm::SHA256,
            value: gem_release.metadata.checksum.clone(),
        }
    }
}
