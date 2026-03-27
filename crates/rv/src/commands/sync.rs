use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use serde::Deserialize;
use tracing::debug;

use crate::commands::run::Invocation;
use crate::{
    GlobalArgs,
    config::Config,
    gemserver::{self, GemRelease, Gemserver},
    resolver::{ResolutionPackage, ResolutionRoot},
};
use rv_gem_types::{
    Platform, ProjectDependency, ReleaseTuple, Requirement, VersionConstraint, VersionPlatform,
};
use rv_lockfile::datatypes::GemfileDotLock;
use std::str::FromStr;
use url::Url;

#[derive(Debug, Args)]
pub struct SyncArgs {
    /// Path to Gemfile
    #[arg(long, env = "BUNDLE_GEMFILE")]
    gemfile: Option<Utf8PathBuf>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] crate::config::Error),
    #[error("{0} is not a valid URL")]
    BadUrl(String),
    #[error("no matching ruby version found")]
    NoMatchingRuby,
    #[error("{gem_name} doesn't exist on {server}")]
    NotFound { gem_name: String, server: String },
    #[error("The gem does not actually have any releases published")]
    NoReleasesPublished,
    #[error(transparent)]
    GemserverError(#[from] gemserver::Error),
    #[error("Could not parse a version from the server: {0}")]
    GemReleaseParse(#[from] gemserver::GemReleaseParse),
    #[error(transparent)]
    Run(#[from] crate::commands::run::Error),
    #[error(transparent)]
    Install(#[from] crate::commands::clean_install::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Parse(#[from] rv_lockfile::ParseErrors),
    #[error(transparent)]
    RequirementError(#[from] rv_gem_types::requirement::RequirementError),
    #[error(transparent)]
    ProjectDependencyError(#[from] rv_gem_types::project_dependency::ProjectDependencyError),
    #[error("A Gemfile file was not found")]
    MissingImplicitGemfile,
    #[error("Gemfile \"{0}\" does not exist")]
    MissingGemfile(String),
    #[error(
        "The gemfile path must be inside a directory with a parent, but it wasn't. Path was {0}"
    )]
    InvalidGemfilePath(String),
    #[error("rv could not resolve the Gemfile:\n\n{0}")]
    ResolutionError(String),
}

type Result<T> = miette::Result<T, Error>;

#[derive(Deserialize, Debug)]
struct Gemfile {
    global_source: String,
    deps: Vec<ProjectDependency>,
    ruby: Option<Vec<VersionConstraint>>,
}

pub(crate) async fn sync(global_args: &GlobalArgs, args: SyncArgs) -> Result<()> {
    let mut config = Config::new(global_args, None)?;

    let (gemfile_path, gemfile_dir) = find_gemfile_path(&args.gemfile)?;

    let script = include_str!("../../scripts/serialize_gemfile.rb").to_string();

    let result = crate::commands::run::capture_run_no_install(
        Invocation::ruby(vec![]),
        &config,
        vec!["--disable-gems".to_string(), "-e".to_string(), script],
        Some(&gemfile_dir),
    )?;

    let output = String::from_utf8(result.stdout).unwrap();
    let gemfile: Gemfile = serde_json::from_str(output.as_str()).expect("not well formatted");

    let global_source = gemfile.global_source;
    let dependencies = gemfile.deps;
    let ruby_requirement = gemfile.ruby.map(Requirement::from);

    let gem_server: Url = global_source
        .parse()
        .map_err(|_| Error::BadUrl(global_source))?;
    let mut gemserver = Gemserver::new(&config, gem_server)?;

    let root = ResolutionRoot {
        package: ResolutionPackage::Gemfile,
        version_platform: VersionPlatform::from_str("0").unwrap(),
        deps: dependencies.clone(),
    };

    let gemfile_ruby = if let Some(requirement) = ruby_requirement {
        config.best_ruby_matching_requirement(&requirement).await?
    } else {
        config.current_ruby().ok_or(Error::NoMatchingRuby)?.version
    };

    gemserver.add_transitive_deps(&root, &gemfile_ruby).await?;

    let gems_to_deps = gemserver.gems_to_deps;

    // OK, now we know all transitive dependencies, and have a dependency graph.
    // Now, translate the dependency constraint list into a PubGrub system, and resolve
    // (i.e. figure out which version of every gem will be used.)
    debug!("Resolving all dependencies via PubGrub");
    let versions_needed = crate::resolver::solve(&root, &gems_to_deps)
        .map_err(|e| Error::ResolutionError(e.to_string()))?;
    debug!("All dependencies resolved");

    let lockfile_path = gemfile_path.with_extension("lock");

    // Make a Gemfile.lock in-memory, install it via `rv ci`.
    let platform = Platform::local();
    let lockfile_builder =
        LockfileBuilder::new(&gemserver.url, versions_needed, platform, dependencies);
    let lockfile = lockfile_builder.lockfile();

    config = Config::with_settings(global_args, Some(gemfile_ruby.clone().into()))?;

    crate::commands::clean_install::install_inline_lockfile(&config, lockfile.clone(), None)
        .await?;

    let lockfile_contents = lockfile.to_string();
    std::fs::write(&lockfile_path, &lockfile_contents)?;

    Ok(())
}

fn find_gemfile_path(gemfile: &Option<Utf8PathBuf>) -> Result<(Utf8PathBuf, Utf8PathBuf)> {
    let Some(gemfile) = gemfile else {
        let gemfile_path = rv_dirs::canonicalize_utf8(Utf8Path::new("Gemfile"))
            .map_err(|_| Error::MissingImplicitGemfile)?;
        let gemfile_dir = gemfile_path
            .parent()
            .expect("if we could canonicalize it, it must have a parent");

        debug!("found Gemfile file in {}", gemfile_dir);
        return Ok((gemfile_path.clone(), gemfile_dir.into()));
    };

    let gemfile_path = rv_dirs::canonicalize_utf8(gemfile)
        .map_err(|_| Error::MissingGemfile(gemfile.to_string()))?;

    let gemfile_dir = gemfile_path
        .parent()
        .ok_or(Error::InvalidGemfilePath(gemfile.to_string()))?;

    debug!("found Gemfile file in {}", gemfile_dir);
    Ok((gemfile_path.clone(), gemfile_dir.into()))
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
    platform: Platform,
    dependencies: Vec<(String, Requirement)>,
}

impl LockfileBuilder {
    pub fn new(
        url: &Url,
        mut versions_needed: Vec<(ReleaseTuple, GemRelease)>,
        platform: Platform,
        dependencies: Vec<ProjectDependency>,
    ) -> Self {
        versions_needed.sort_by_key(|k| k.0.clone());
        let gemserver_remote = url.to_string();
        let dependencies: Vec<_> = dependencies
            .into_iter()
            .map(|d| (d.name.to_string(), d.requirement))
            .collect();
        Self {
            gemserver_remote,
            versions_needed,
            platform,
            dependencies,
        }
    }

    /// Create an in-memory Gemfile.lock that views/borrows its data from this builder.
    pub fn lockfile(&self) -> GemfileDotLock<'_> {
        let mut lockfile = rv_lockfile::datatypes::GemfileDotLock::default();
        let mut gem_section = rv_lockfile::datatypes::GemSection {
            remote: Some(&self.gemserver_remote),
            specs: Vec::new(),
        };
        let mut checksums = vec![];
        for (release_tuple, gem_release) in &self.versions_needed {
            let mut deps = gem_release.deps.clone();
            deps.sort_by_key(|d| d.name.clone());
            let spec = Self::spec_for_gem_dep(release_tuple, &deps);
            gem_section.specs.push(spec);
            let checksum = Self::checksum_for_spec(release_tuple, gem_release);
            checksums.push(checksum);
        }
        lockfile.gem.push(gem_section);
        lockfile.platforms.push(self.platform.clone());
        for (name, requirement) in &self.dependencies {
            let range = rv_lockfile::datatypes::GemRange {
                name,
                requirement: requirement.clone(),
                nonstandard: false,
            };

            lockfile.dependencies.push(range);
        }

        lockfile
    }

    fn spec_for_gem_dep(
        release_tuple: &ReleaseTuple,
        deps: &[ProjectDependency],
    ) -> rv_lockfile::datatypes::Spec {
        rv_lockfile::datatypes::Spec {
            deps: deps.to_vec(),
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
