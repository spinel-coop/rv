use std::str::FromStr;

use reqwest::Client;
use rv_gem_types::Platform;
use rv_lockfile::datatypes::SemverConstraint;
use rv_ruby::version::ParseVersionError;
use rv_version::Version;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use url::Url;

use crate::http_client::rv_http_client;

pub struct Gemserver {
    pub url: Url,
    client: Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("The requested gem {gem} was not found on the RubyGems server {gem_server}")]
    GemNotFound { gem: String, gem_server: Url },
}

impl Gemserver {
    pub fn new(url: Url) -> Result<Self, Error> {
        let client = rv_http_client("install")?;
        Ok(Self { url, client })
    }

    /// Returns the response body from the server SERVER/info/GEM_NAME.
    /// You probably want to call [`parse_version_from_body`] on that string.
    /// This function doesn't parse the response, so that the parser doesn't have to copy any strings.
    /// Whoever calls this should own the response, and then the parser will borrow &strs from the response.
    pub async fn get_versions_for_gem(&self, gem: &str) -> Result<String, Error> {
        let mut url = self.url.clone();
        url.set_path(&format!("info/{}", gem));
        let index_body = self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        if index_body.is_empty() {
            return Err(Error::GemNotFound {
                gem: gem.to_owned(),
                gem_server: self.url.to_owned(),
            });
        }
        Ok(index_body)
    }
}

/// Given a response body from the server SERVER/info/GEM_NAME,
/// parse it into a list of versions.
pub fn parse_version_from_body(
    index_body: &str,
) -> Result<Vec<VersionAvailable>, VersionAvailableParse> {
    index_body
        .lines()
        .filter_map(|line| {
            if line == "---" {
                return None;
            }
            Some(VersionAvailable::parse(line))
        })
        .collect()
}

/// All the information about a versiom of a gem available on some Gemserver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionAvailable {
    pub version: Version,
    pub platform: Platform,
    pub deps: Vec<Dep>,
    pub metadata: Metadata,
}

#[derive(Debug, thiserror::Error)]
pub enum VersionAvailableParse {
    #[error("Missing a space")]
    MissingSpace,
    #[error("Missing a pipe")]
    MissingPipe,
    #[error("Missing a colon")]
    MissingColon,
    #[error("Missing a colon in metadata field: {0}")]
    MissingMetadataColon(String),
    #[error(transparent)]
    ParseVersionError(#[from] ParseVersionError),
    #[error("Unknown semver constraint type {0}")]
    UnknownSemverType(String),
    #[error("Unknown metadata key {key} in metadata field: {metadata}")]
    UnknownMetadataKey { key: String, metadata: String },
    #[error("Invalid checksum in metadata field {metadata}: {source}")]
    InvalidChecksum {
        source: hex::FromHexError,
        metadata: String,
    },
    #[error("Invalid constraint in metadata field {metadata}: {source}")]
    MetadataConstraintParse {
        source: Box<VersionAvailableParse>,
        metadata: String,
    },
    #[error(transparent)]
    InvalidVersion(#[from] rv_version::VersionError),
}

impl VersionAvailable {
    /// Parses from a string like this:
    /// 2.2.2 actionmailer:= 2.2.2,actionpack:= 2.2.2,activerecord:= 2.2.2,activeresource:= 2.2.2,activesupport:=
    /// 2.2.2,rake:>= 0.8.3|checksum:84fd0ee92f92088cff81d1a4bcb61306bd4b7440b8634d7ac3d1396571a2133f
    fn parse(line: &str) -> std::result::Result<Self, VersionAvailableParse> {
        let (v, rest) = line
            .split_once(' ')
            .ok_or(VersionAvailableParse::MissingSpace)?;
        let version = v;
        let (deps, metadata) = rest
            .split_once('|')
            .ok_or(VersionAvailableParse::MissingPipe)?;

        let deps: Vec<_> = if deps.is_empty() {
            Default::default()
        } else {
            deps.split(',')
                .map(|dep| {
                    let (gem_name, constraints) = dep
                        .split_once(':')
                        .ok_or(VersionAvailableParse::MissingColon)?;

                    let version_constraint = constraints
                        .split('&')
                        .map(VersionConstraint::from_str)
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    Ok::<_, VersionAvailableParse>(Dep {
                        gem_name: gem_name.to_owned(),
                        version_constraints: version_constraint.into(),
                    })
                })
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let metadata = parse_metadata(metadata)?;

        let (version, platform) = if let Some((version, platform)) =
            rv_gem_types::platform::version_platform_split(version)
        {
            (version, platform)
        } else {
            (version.parse()?, Platform::Ruby)
        };
        Ok(VersionAvailable {
            version,
            platform,
            deps,
            metadata,
        })
    }
}

fn parse_metadata(metadata: &str) -> Result<Metadata, VersionAvailableParse> {
    let mut out = Metadata::default();
    for md_str in metadata.split(',') {
        if md_str.is_empty() {
            continue;
        }
        let (k, v) = md_str
            .split_once(':')
            .ok_or_else(|| VersionAvailableParse::MissingMetadataColon(md_str.to_owned()))?;
        match k {
            "checksum" => {
                out.checksum =
                    hex::decode(v).map_err(|err| VersionAvailableParse::InvalidChecksum {
                        source: err,
                        metadata: md_str.to_owned(),
                    })?;
            }
            "ruby" => {
                out.ruby = v
                    .split('&')
                    .map(VersionConstraint::from_str)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| VersionAvailableParse::MetadataConstraintParse {
                        source: Box::new(err),
                        metadata: md_str.to_owned(),
                    })?;
            }
            "rubygems" => {
                out.rubygems = v
                    .split('&')
                    .map(VersionConstraint::from_str)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| VersionAvailableParse::MetadataConstraintParse {
                        source: Box::new(err),
                        metadata: md_str.to_owned(),
                    })?;
            }
            _ => {
                return Err(VersionAvailableParse::UnknownMetadataKey {
                    key: k.to_owned(),
                    metadata: md_str.to_owned(),
                });
            }
        }
    }
    Ok(out)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Dep {
    /// What gem this dependency uses.
    pub gem_name: String,
    /// Constraints on what version of the gem can be used.
    pub version_constraints: VersionConstraints,
}

impl std::fmt::Debug for Dep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.gem_name, self.version_constraints)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VersionConstraints {
    pub inner: Vec<VersionConstraint>,
}

impl std::fmt::Debug for VersionConstraints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}

impl From<Vec<VersionConstraint>> for VersionConstraints {
    fn from(constraints: Vec<VersionConstraint>) -> Self {
        Self { inner: constraints }
    }
}

impl From<VersionConstraints> for Vec<VersionConstraint> {
    fn from(constraints: VersionConstraints) -> Self {
        constraints.inner
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde_as]
pub struct Metadata {
    #[serde_as(as = "serde_with::hex::Hex")]
    pub checksum: Vec<u8>,
    pub ruby: Vec<VersionConstraint>,
    pub rubygems: Vec<VersionConstraint>,
}

impl std::fmt::Debug for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Metadata")
            .field("checksum", &hex::encode(&self.checksum))
            .field("ruby", &self.ruby)
            .field("rubygems", &self.rubygems)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VersionConstraint {
    pub constraint_type: SemverConstraint,
    pub version: Version,
}

impl std::fmt::Debug for VersionConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} {}", self.constraint_type, self.version)
    }
}

impl FromStr for VersionConstraint {
    type Err = VersionAvailableParse;

    fn from_str(constr: &str) -> Result<Self, Self::Err> {
        let (semver_constr, v) = constr
            .split_once(' ')
            .ok_or(VersionAvailableParse::MissingSpace)?;
        Ok::<_, VersionAvailableParse>(VersionConstraint {
            constraint_type: semver_constr
                .parse()
                .map_err(VersionAvailableParse::UnknownSemverType)?,
            version: v.parse().map_err(VersionAvailableParse::InvalidVersion)?,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser() {
        for (expected_version, input) in [
            (
                "0".parse::<Version>().unwrap(),
                "0 activemodel-globalid:>= 0,activesupport:>= 4.1.0|checksum:76c450d211f74a575fd4d32d08e5578d829a419058126fbb3b89ad5bf3621c94,ruby:>= 1.9.3",
            ),
            (
                "0.0.0".parse().unwrap(),
                "0.0.0 |checksum:505c6770a5ec896244d31d7eac08663696d22140493ddb820f66d12670b669d2",
            ),
            (
                "8.1.2".parse().unwrap(),
                "8.1.2 activesupport:= 8.1.2,globalid:>= 0.3.6|checksum:908dab3713b101859536375819f4156b07bdf4c232cc645e7538adb9e302f825,ruby:>= 3.2.0",
            ),
        ] {
            let actual = VersionAvailable::parse(input).unwrap();
            assert_eq!(expected_version, actual.version);
        }
    }

    #[test]
    fn test_body_parser() {
        let resp = "---
2.2.2 actionmailer:= 2.2.2,actionpack:= 2.2.2,activerecord:= 2.2.2,activeresource:= 2.2.2,activesupport:= 2.2.2,rake:>= 0.8.3|checksum:84fd0ee92f92088cff81d1a4bcb61306bd4b7440b8634d7ac3d1396571a2133f
2.3.2 actionmailer:= 2.3.2,actionpack:= 2.3.2,activerecord:= 2.3.2,activeresource:= 2.3.2,activesupport:= 2.3.2,rake:>= 0.8.3|checksum:ac61e0356987df34dbbafb803b98f153a663d3878a31f1db7333b7cd987fd044";
        let actual_parsed_response = parse_version_from_body(resp).unwrap();
        assert_eq!(actual_parsed_response.len(), 2);
        insta::assert_debug_snapshot!(actual_parsed_response);
    }
}
