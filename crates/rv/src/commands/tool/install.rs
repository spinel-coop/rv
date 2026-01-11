use owo_colors::OwoColorize;
use rv_ruby::version::{ParseVersionError, RubyVersion};
use url::Url;

use crate::{config::Config, http_client::rv_http_client};

const GEM_COOP: &str = "https://gem.coop/";

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("{0} is not a valid URL")]
    BadUrl(String),
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
    #[error("The requested gem {gem} was not found on the RubyGems server {gem_server}")]
    GemNotFound { gem: String, gem_server: Url },
    #[error("Could not parse a version from the server: {0}")]
    VersionAvailableParse(#[from] VersionAvailableParse),
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
    let client = rv_http_client("install")?;
    let mut url = args.gem_server.clone();
    url.set_path(&format!("info/{}", args.gem));
    let index_body = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let mut versions: Vec<VersionAvailable> = Vec::new();
    for line in index_body.lines() {
        if line == "---" {
            continue;
        }
        versions.push(VersionAvailable::parse(line)?);
    }
    if versions.is_empty() {
        return Err(Error::GemNotFound {
            gem: args.gem,
            gem_server: args.gem_server,
        });
    }

    eprintln!("{versions:#?}");
    tracing::debug!("Found {} versions for the gem", versions.len());
    println!("{}: install {}", "todo".yellow(), args.gem);
    Ok(())
}

/// All the information about a versiom of a gem available on some Gemserver.
#[derive(Debug)]
struct VersionAvailable<'i> {
    // TODO: This should probably be its own type, GemVersion.
    version: RubyVersion,
    deps: Vec<Dep<'i>>,
    metadata: Metadata<'i>,
}

#[derive(Debug, thiserror::Error)]
pub enum VersionAvailableParse {
    #[error("Missing a space")]
    MissingSpace,
    #[error("Missing a pipe")]
    MissingPipe,
    #[error("Missing a colon")]
    MissingColon,
    #[error(transparent)]
    ParseVersionError(#[from] ParseVersionError),
}

impl<'i> VersionAvailable<'i> {
    /// Parses from a string like this:
    /// 2.2.2 actionmailer:= 2.2.2,actionpack:= 2.2.2,activerecord:= 2.2.2,activeresource:= 2.2.2,activesupport:=
    /// 2.2.2,rake:>= 0.8.3|checksum:84fd0ee92f92088cff81d1a4bcb61306bd4b7440b8634d7ac3d1396571a2133f
    fn parse(line: &'i str) -> std::result::Result<Self, VersionAvailableParse> {
        eprintln!("{line}");
        let (v, rest) = line
            .split_once(' ')
            .ok_or(VersionAvailableParse::MissingSpace)?;
        let version = v.parse()?;
        let (deps, metadata) = rest
            .split_once('|')
            .ok_or(VersionAvailableParse::MissingPipe)?;
        let deps: Vec<_> = deps
            .split(',')
            .map(|dep| {
                let (gem_name, version_constraint) = dep
                    .split_once(":")
                    .ok_or(VersionAvailableParse::MissingColon)?;
                Ok::<_, VersionAvailableParse>(Dep {
                    gem_name,
                    version_constraint,
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let metadata: Metadata<'_> =
            metadata
                .split(',')
                .fold(Metadata::default(), |mut partial, md_str| {
                    let (k, v) = md_str.split_once(':').unwrap();
                    if k == "checksum" {
                        partial.checksum = hex::decode(v).unwrap();
                    } else if k == "ruby" {
                        partial.ruby = Some(v);
                    } else if k == "rubygems" {
                        partial.rubygems = Some(v);
                    } else {
                        eprintln!("unexpected key {k}, {md_str}");
                        panic!();
                    }
                    partial
                });
        Ok(VersionAvailable {
            version,
            deps,
            metadata,
        })
    }
}

#[derive(Debug)]
struct Dep<'i> {
    gem_name: &'i str,
    version_constraint: &'i str,
}

#[derive(Debug, Default)]
struct Metadata<'i> {
    checksum: Vec<u8>,
    ruby: Option<&'i str>,
    rubygems: Option<&'i str>,
}
