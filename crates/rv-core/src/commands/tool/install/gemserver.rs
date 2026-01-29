use std::str::FromStr;

use reqwest::Client;
use rv_gem_types::{Platform, VersionPlatform};
use rv_lockfile::datatypes::SemverConstraint;
use rv_version::Version;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use url::Url;

use crate::{commands::tool::install::GemName, http_client::rv_http_client};

pub struct Gemserver {
    pub url: Url,
    client: Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("The requested gem {gem} was not found on the RubyGems server {gem_server}")]
    GemNotFound { gem: GemName, gem_server: Url },
}

impl Gemserver {
    pub fn new(url: Url) -> Result<Self, Error> {
        let client = rv_http_client("install")?;
        Ok(Self { url, client })
    }

    /// Returns the response body from the server SERVER/info/GEM_NAME.
    /// You probably want to call [`parse_release_from_body`] on that string.
    /// This function doesn't parse the response, so that the parser doesn't have to copy any strings.
    /// Whoever calls this should own the response, and then the parser will borrow &strs from the response.
    pub async fn get_releases_for_gem(&self, gem: &str) -> Result<String, Error> {
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
pub fn parse_release_from_body(index_body: &str) -> Result<Vec<GemRelease>, GemReleaseParse> {
    index_body
        .lines()
        .filter_map(|line| {
            if line == "---" {
                return None;
            }

            let gem_release = GemRelease::parse(line);

            if let Ok(release) = &gem_release
                && !release.platform().is_local()
            {
                return None;
            }

            Some(gem_release)
        })
        .collect()
}

/// All the information about a release of a gem available on some Gemserver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GemRelease {
    pub version_platform: VersionPlatform,
    pub deps: Vec<Dep>,
    pub metadata: Metadata,
}

impl GemRelease {
    pub fn version_platform(&self) -> &VersionPlatform {
        &self.version_platform
    }

    pub fn version(&self) -> &Version {
        &self.version_platform.version
    }

    pub fn platform(&self) -> &Platform {
        &self.version_platform.platform
    }
}

impl From<GemRelease> for VersionPlatform {
    fn from(value: GemRelease) -> Self {
        value.version_platform
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GemReleaseParse {
    #[error("Missing a space")]
    MissingSpace,
    #[error("Missing a pipe")]
    MissingPipe,
    #[error("Missing a colon")]
    MissingColon,
    #[error("Missing a colon in metadata field: {0}")]
    MissingMetadataColon(String),
    #[error(transparent)]
    InvalidRubyVersion(#[from] rv_ruby::version::ParseVersionError),
    #[error(transparent)]
    InvalidVersion(#[from] rv_version::VersionError),
    #[error("Invalid release: {0}")]
    InvalidRelease(String),
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
        source: Box<GemReleaseParse>,
        metadata: String,
    },
}

impl GemRelease {
    /// Parses from a string like this:
    /// 2.2.2 actionmailer:= 2.2.2,actionpack:= 2.2.2,activerecord:= 2.2.2,activeresource:= 2.2.2,activesupport:=
    /// 2.2.2,rake:>= 0.8.3|checksum:84fd0ee92f92088cff81d1a4bcb61306bd4b7440b8634d7ac3d1396571a2133f
    fn parse(line: &str) -> std::result::Result<Self, GemReleaseParse> {
        let (v, rest) = line.split_once(' ').ok_or(GemReleaseParse::MissingSpace)?;
        let version = v;
        let (deps, metadata) = rest.split_once('|').ok_or(GemReleaseParse::MissingPipe)?;

        let deps: Vec<_> = if deps.is_empty() {
            Default::default()
        } else {
            deps.split(',')
                .map(|dep| {
                    let (gem_name, constraints) =
                        dep.split_once(':').ok_or(GemReleaseParse::MissingColon)?;

                    let version_constraint = constraints
                        .split('&')
                        .map(VersionConstraint::from_str)
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    Ok::<_, GemReleaseParse>(Dep {
                        gem_name: gem_name.to_owned(),
                        version_constraints: version_constraint.into(),
                    })
                })
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        let metadata = parse_metadata(metadata)?;

        let version_platform = VersionPlatform::from_str(version)
            .map_err(|_| GemReleaseParse::InvalidRelease(version.to_string()))?;

        Ok(GemRelease {
            version_platform,
            deps,
            metadata,
        })
    }

    pub fn full_name(&self) -> String {
        self.version_platform().to_string()
    }
}

fn parse_metadata(metadata: &str) -> Result<Metadata, GemReleaseParse> {
    let mut out = Metadata::default();
    for md_str in metadata.split(',') {
        if md_str.is_empty() {
            continue;
        }
        let (k, v) = md_str
            .split_once(':')
            .ok_or_else(|| GemReleaseParse::MissingMetadataColon(md_str.to_owned()))?;
        match k {
            "checksum" => {
                out.checksum = hex::decode(v).map_err(|err| GemReleaseParse::InvalidChecksum {
                    source: err,
                    metadata: md_str.to_owned(),
                })?;
            }
            "ruby" => {
                out.ruby = v
                    .split('&')
                    .map(VersionConstraint::from_str)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| GemReleaseParse::MetadataConstraintParse {
                        source: Box::new(err),
                        metadata: md_str.to_owned(),
                    })?;
            }
            "rubygems" => {
                out.rubygems = v
                    .split('&')
                    .map(VersionConstraint::from_str)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| GemReleaseParse::MetadataConstraintParse {
                        source: Box::new(err),
                        metadata: md_str.to_owned(),
                    })?;
            }
            _ => {
                return Err(GemReleaseParse::UnknownMetadataKey {
                    key: k.to_owned(),
                    metadata: md_str.to_owned(),
                });
            }
        }
    }
    Ok(out)
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dep {
    /// What gem this dependency uses.
    pub gem_name: GemName,
    /// Constraints on what version of the gem can be used.
    pub version_constraints: VersionConstraints,
}

impl std::fmt::Debug for Dep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.gem_name, self.version_constraints)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    type Err = GemReleaseParse;

    fn from_str(constr: &str) -> Result<Self, Self::Err> {
        let (semver_constr, v) = constr
            .split_once(' ')
            .ok_or(GemReleaseParse::MissingSpace)?;
        Ok::<_, GemReleaseParse>(VersionConstraint {
            constraint_type: semver_constr
                .parse()
                .map_err(GemReleaseParse::UnknownSemverType)?,
            version: v.parse().map_err(GemReleaseParse::InvalidVersion)?,
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
            let actual = GemRelease::parse(input).unwrap();
            assert_eq!(&expected_version, actual.version());
        }
    }

    #[test]
    fn test_body_parser() {
        let resp = "---
2.2.2 actionmailer:= 2.2.2,actionpack:= 2.2.2,activerecord:= 2.2.2,activeresource:= 2.2.2,activesupport:= 2.2.2,rake:>= 0.8.3|checksum:84fd0ee92f92088cff81d1a4bcb61306bd4b7440b8634d7ac3d1396571a2133f
2.3.2 actionmailer:= 2.3.2,actionpack:= 2.3.2,activerecord:= 2.3.2,activeresource:= 2.3.2,activesupport:= 2.3.2,rake:>= 0.8.3|checksum:ac61e0356987df34dbbafb803b98f153a663d3878a31f1db7333b7cd987fd044";
        let actual_parsed_response = parse_release_from_body(resp).unwrap();
        assert_eq!(actual_parsed_response.len(), 2);
        insta::assert_debug_snapshot!(actual_parsed_response);
    }

    #[test]
    fn test_sort_version_available() {
        let resp = "---
1.19.0-aarch64-linux-gnu racc:~> 1.4|checksum:11a97ecc3c0e7e5edcf395720b10860ef493b768f6aa80c539573530bc933767,ruby:< 4.1.dev&>= 3.2,rubygems:>= 3.3.22
1.19.0-aarch64-linux-musl racc:~> 1.4|checksum:eb70507f5e01bc23dad9b8dbec2b36ad0e61d227b42d292835020ff754fb7ba9,ruby:< 4.1.dev&>= 3.2,rubygems:>= 3.3.22
1.19.0-arm-linux-gnu racc:~> 1.4|checksum:572a259026b2c8b7c161fdb6469fa2d0edd2b61cd599db4bbda93289abefbfe5,ruby:< 4.1.dev&>= 3.2,rubygems:>= 3.3.22
1.19.0-arm-linux-musl racc:~> 1.4|checksum:23ed90922f1a38aed555d3de4d058e90850c731c5b756d191b3dc8055948e73c,ruby:< 4.1.dev&>= 3.2,rubygems:>= 3.3.22
1.19.0-arm64-darwin racc:~> 1.4|checksum:0811dfd936d5f6dd3f6d32ef790568bf29b2b7bead9ba68866847b33c9cf5810,ruby:< 4.1.dev&>= 3.2
1.19.0-java racc:~> 1.4|checksum:5f3a70e252be641d8a4099f7fb4cc25c81c632cb594eec9b4b8f2ca8be4374f3,ruby:>= 3.2
1.19.0-x64-mingw-ucrt racc:~> 1.4|checksum:05d7ed2d95731edc9bef2811522dc396df3e476ef0d9c76793a9fca81cab056b,ruby:< 4.1.dev&>= 3.2
1.19.0-x86_64-darwin racc:~> 1.4|checksum:1dad56220b603a8edb9750cd95798bffa2b8dd9dd9aa47f664009ee5b43e3067,ruby:< 4.1.dev&>= 3.2
1.19.0-x86_64-linux-gnu racc:~> 1.4|checksum:f482b95c713d60031d48c44ce14562f8d2ce31e3a9e8dd0ccb131e9e5a68b58c,ruby:< 4.1.dev&>= 3.2,rubygems:>= 3.3.22
1.19.0-x86_64-linux-musl racc:~> 1.4|checksum:1c4ca6b381622420073ce6043443af1d321e8ed93cc18b08e2666e5bd02ffae4,ruby:< 4.1.dev&>= 3.2,rubygems:>= 3.3.22
1.19.0 mini_portile2:~> 2.8.2,racc:~> 1.4|checksum:e304d21865f62518e04f2bf59f93bd3a97ca7b07e7f03952946d8e1c05f45695,ruby:>= 3.2";

        let actual_parsed_response = parse_release_from_body(resp).unwrap();
        assert_eq!(actual_parsed_response.len(), 2);

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let expected_release = "1.19.0-arm64-darwin";
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let expected_release = "1.19.0-x86_64-darwin";
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let expected_release = "1.19.0-aarch64-linux-gnu";
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let expected_release = "1.19.0-x86_64-linux-gnu";

        assert_eq!(
            actual_parsed_response
                .iter()
                .map(|gr| gr.version_platform())
                .max()
                .unwrap()
                .to_string(),
            expected_release
        );
    }
}
