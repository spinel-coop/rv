use owo_colors::OwoColorize;
use rv_lockfile::datatypes::SemverConstraint;
use rv_ruby::{
    request::RubyRequest,
    version::{ParseVersionError, RubyVersion},
};
use url::Url;

use crate::{
    commands::tool::install::gemserver::Gemserver, config::Config, http_client::rv_http_client,
};

mod gemserver;

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

pub async fn install(_config: &Config, gem: String) -> Result<()> {
    let args = InnerArgs::new(gem)?;
    let gemserver = Gemserver::new(args.gem_server)?;
    let versions_resp = gemserver.get_versions_for_gem(&args.gem).await?;
    let versions = gemserver::parse_version_from_body(&versions_resp)?;

    tracing::info!("Found {} versions for the gem", versions.len());
    let most_recent_version = versions.iter().max_by_key(|x| &x.version).unwrap();
    eprintln!(
        "{}: Install version {}",
        "TODO".yellow(),
        most_recent_version.version
    );
    eprintln!(
        "Metadata: ruby={:?} rubyversions={:?}",
        most_recent_version.metadata.ruby, most_recent_version.metadata.rubygems
    );
    eprintln!("Deps: {:#?}", most_recent_version.deps);
    Ok(())
}
