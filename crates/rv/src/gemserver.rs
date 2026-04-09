use futures_util::{StreamExt, stream::FuturesUnordered};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;
use std::{rc::Rc, sync::Mutex};

use rv_gem_types::requirement::{Requirement, VersionConstraint};
use rv_gem_types::{Platform, ProjectDependency, VersionPlatform};
use rv_ruby::version::RubyVersion;
use rv_version::Version;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::debug;
use url::Url;

use crate::config::Config;
use crate::gemserver::http_fetcher::HttpFetcher;
use crate::gemserver::storage::{FilesystemStorage, Storage};
use crate::gemserver::updater::Updater;
use crate::resolver::GemDepsMap;

pub mod http_fetcher;
pub mod storage;
pub mod updater;

pub struct Gemserver {
    pub url: Url,
    updater: Arc<Updater>,
    storage: Arc<dyn Storage>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    HttpError(#[from] http_fetcher::Error),
    #[error(transparent)]
    StorageError(#[from] storage::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("Could not parse a version from the server: {0}")]
    GemReleaseParse(#[from] GemReleaseParse),
    #[error("Could not create the cache dir: {0}")]
    CouldNotCreateCacheDir(std::io::Error),
    #[error("The url {url} unexpectedly returned an empty response")]
    EmptyResponse { url: Url },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Gemserver {
    pub fn new(config: &Config, mut url: Url) -> Result<Self> {
        let cache_dir = config
            .cache
            .shard(rv_cache::CacheBucket::GemDeps, "compact_index")
            .into_path_buf();

        fs_err::create_dir_all(&cache_dir).map_err(Error::CouldNotCreateCacheDir)?;

        let client = HttpFetcher::new("install")?;
        let storage = FilesystemStorage::new(cache_dir.into());
        let updater = Updater::new(client);

        // Add a trailing slash to the url if not already there. Otherwise, if the gemserver is
        // namespaced, the namespace is ignored because joining url's requires the base url with
        // have a trailing slash, and we join url's to construct compact index endpoints
        url.path_segments_mut()
            .expect("this url cannot be a base")
            .push("");

        Ok(Self {
            url,
            storage: Arc::new(storage),
            updater: Arc::new(updater),
        })
    }

    /// Returns the response body from the server SERVER/info/GEM_NAME.
    /// Fetches the file using etag/range requests if it's already there.
    /// Otherwise fetches a fresh copy.
    /// You probably want to call [`parse_release_from_body`] on the returned string.
    /// This function doesn't parse the response, so that the parser doesn't have to copy any strings.
    /// Whoever calls this should own the response, and then the parser will borrow &strs from the response.
    pub async fn get_releases_for_gem(&self, gem: &str) -> Result<String> {
        let info_key = format!("info/{}", gem);
        let info_url = self.url.join(&info_key).expect("valid info URL");

        let blob = if let Ok(blob) = self.storage.read_blob(&info_key).await {
            self.updater.update(info_url.as_str(), blob).await
        } else {
            self.updater.fetch(info_url.as_str()).await
        }
        .map_err(|err| {
            if matches!(err, Error::StorageError(storage::Error::EmptyContent)) {
                Error::EmptyResponse {
                    url: self.url.to_owned(),
                }
            } else {
                err
            }
        })?;

        self.storage.write_blob(&info_key, &blob).await?;

        let index_body = String::from_utf8_lossy(&blob.content).to_string();

        Ok(index_body)
    }

    async fn fetch(&self, req: String) -> Result<((String, Vec<GemRelease>), Vec<String>)> {
        debug!("Fetching {req}");
        let dep_info_resp = self.get_releases_for_gem(&req).await?;
        let dep_versions = parse_release_from_body(&dep_info_resp)?;
        let transitive_deps = dep_versions
            .iter()
            .flat_map(|d| d.clone().deps.into_iter().map(|d| d.name))
            .collect();
        Ok(((req, dep_versions), transitive_deps))
    }

    pub async fn query_all_gem_deps(
        &self,
        root: &GemRelease,
        ruby_to_use: &RubyVersion,
    ) -> Result<GemDepsMap> {
        let results = Rc::new(Mutex::new(GemDepsMap::default()));
        let mut in_flight = FuturesUnordered::new();
        let seen_requests = Rc::new(Mutex::new(HashSet::<String>::new()));

        // Initial requests
        for d in &root.deps {
            let req = d.name.clone();
            debug!("Queuing {req}");
            in_flight.push(self.fetch(req))
        }

        // Keep fetching new dependencies we discover.
        while let Some(res) = in_flight.next().await {
            let ((dep_name, dep_info), new_deps) = res?;
            {
                let mut results = results.lock().expect("Lock poisoned");
                // Skip possible versions that are incompatible with our
                // chosen Ruby version.
                // We should filter these out now, so that we minimize the number
                // of deps that PubGrub has to consider.
                let candidate_versions: HashMap<VersionPlatform, GemRelease> = dep_info
                    .into_iter()
                    .filter(|release| {
                        release
                            .metadata
                            .ruby
                            .satisfied_by(&rv_version::Version::from(ruby_to_use))
                    })
                    .map(|release| (release.version_platform.clone(), release))
                    .collect();
                results.insert(dep_name, candidate_versions);
            }

            for req in new_deps {
                if seen_requests
                    .lock()
                    .expect("Lock poisoned")
                    .insert(req.clone())
                {
                    debug!("Queuing {req}");
                    in_flight.push(self.fetch(req));
                }
            }
        }

        Ok(Rc::into_inner(results).unwrap().into_inner().unwrap())
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
    #[error("Invalid checksum in metadata field {metadata}")]
    InvalidChecksum {
        source: hex::FromHexError,
        metadata: String,
    },
    #[error("Invalid constraint in metadata field {metadata}")]
    MetadataConstraintParse {
        source: Box<GemReleaseParse>,
        metadata: String,
    },
}

pub type ParseResult<T> = std::result::Result<T, GemReleaseParse>;

/// Given a response body from the server SERVER/info/GEM_NAME,
/// parse it into a list of versions.
pub fn parse_release_from_body(index_body: &str) -> ParseResult<Vec<GemRelease>> {
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
    pub deps: Vec<ProjectDependency>,
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

impl GemRelease {
    /// Parses from a string like this:
    /// 2.2.2 actionmailer:= 2.2.2,actionpack:= 2.2.2,activerecord:= 2.2.2,activeresource:= 2.2.2,activesupport:=
    /// 2.2.2,rake:>= 0.8.3|checksum:84fd0ee92f92088cff81d1a4bcb61306bd4b7440b8634d7ac3d1396571a2133f
    fn parse(line: &str) -> ParseResult<Self> {
        let (v, rest) = line.split_once(' ').ok_or(GemReleaseParse::MissingSpace)?;
        let version = v;
        let (deps, metadata) = rest.split_once('|').ok_or(GemReleaseParse::MissingPipe)?;

        let deps: Vec<_> = if deps.is_empty() {
            Default::default()
        } else {
            deps.split(',')
                .map(|dep| {
                    let (name, constraints) =
                        dep.split_once(':').ok_or(GemReleaseParse::MissingColon)?;

                    let version_constraint = constraints
                        .split('&')
                        .map(parse_version_constraint)
                        .collect::<ParseResult<Vec<_>>>()?;
                    Ok(ProjectDependency {
                        name: name.to_owned(),
                        requirement: version_constraint.into(),
                    })
                })
                .collect::<ParseResult<Vec<_>>>()?
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

fn parse_metadata(metadata: &str) -> ParseResult<Metadata> {
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
                    .map(parse_version_constraint)
                    .collect::<ParseResult<Vec<_>>>()
                    .map_err(|err| GemReleaseParse::MetadataConstraintParse {
                        source: Box::new(err),
                        metadata: md_str.to_owned(),
                    })?
                    .into();
            }
            "rubygems" => {
                out.rubygems = v
                    .split('&')
                    .map(parse_version_constraint)
                    .collect::<ParseResult<Vec<_>>>()
                    .map_err(|err| GemReleaseParse::MetadataConstraintParse {
                        source: Box::new(err),
                        metadata: md_str.to_owned(),
                    })?
                    .into();
            }
            "executables" => {
                //Unused for now
            }
            "licenses" => {
                //Unused for now
            }
            "published_at" => {
                //Unused for now
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

fn parse_version_constraint(constr: &str) -> ParseResult<VersionConstraint> {
    if constr.is_empty() {
        return Ok(VersionConstraint::default());
    }

    let (op, v) = constr
        .split_once(' ')
        .ok_or(GemReleaseParse::MissingSpace)?;

    Ok(VersionConstraint {
        operator: op.parse().map_err(GemReleaseParse::UnknownSemverType)?,
        version: v.parse().map_err(GemReleaseParse::InvalidVersion)?,
    })
}

pub type GemName = String;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde_as]
pub struct Metadata {
    #[serde_as(as = "serde_with::hex::Hex")]
    pub checksum: Vec<u8>,
    pub ruby: Requirement,
    pub rubygems: Requirement,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            checksum: vec![],
            ruby: Requirement {
                constraints: vec![],
            },
            rubygems: Requirement {
                constraints: vec![],
            },
        }
    }
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
            (
                "0.5.1".parse().unwrap(),
                "0.5.1 |checksum:f8eb8f78342e3366509d2acb6ee87afec77e49c1545c7e7a76bdaab0f820db46,ruby:>= 1.8.1,rubygems:",
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
1.19.0-aarch64-mingw-ucrt racc:~> 1.4|checksum:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,ruby:< 4.1.dev&>= 3.2
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
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        let expected_release = "1.19.0-x64-mingw-ucrt";
        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        let expected_release = "1.19.0-aarch64-mingw-ucrt";

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
