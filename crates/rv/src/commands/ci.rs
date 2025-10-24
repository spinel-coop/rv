use bytes::Bytes;
use camino::Utf8PathBuf;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use reqwest::Client;
use rv_lockfile::datatypes::GemSection;
use rv_lockfile::datatypes::GemVersion;
use rv_lockfile::datatypes::GemfileDotLock;
use rv_lockfile::datatypes::Spec;
use tracing::debug;
use tracing::info;
use url::Url;

use crate::config::Config;
use std::io;
use std::path::PathBuf;

#[derive(clap_derive::Args)]
pub struct CiArgs {
    /// Maximum number of downloads that can be in flight at once.
    #[arg(short, long, default_value = "10")]
    pub max_concurrent_requests: usize,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] rv_lockfile::ParseErrors),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("Invalid remote URL")]
    BadRemote {
        remote: String,
        err: url::ParseError,
    },
    #[error(transparent)]
    UrlError(#[from] url::ParseError),
    #[error("Could not read install directory from Bundler")]
    BadBundlePath,
    #[error("Failed to unpack tarball path {0}")]
    InvalidTarballPath(PathBuf),
}

type Result<T> = std::result::Result<T, Error>;

pub async fn ci(config: &Config, args: CiArgs) -> Result<()> {
    let lockfile_path;
    if let Some(path) = &config.gemfile {
        lockfile_path = format!("{}.lock", path.clone()).into();
    } else {
        lockfile_path = "Gemfile.lock".into();
    }
    ci_inner(lockfile_path, &config.cache, args.max_concurrent_requests).await
}

async fn ci_inner(
    lockfile_path: Utf8PathBuf,
    cache: &rv_cache::Cache,
    max_concurrent_requests: usize,
) -> Result<()> {
    let lockfile_contents = std::fs::read_to_string(lockfile_path)?;
    let lockfile = rv_lockfile::parse(&lockfile_contents)?;
    let gems = download_gems(lockfile, cache, max_concurrent_requests).await?;
    install_gems(gems)?;
    Ok(())
}

fn find_bundle_path() -> Result<Utf8PathBuf> {
    let bundle_path = std::process::Command::new("ruby")
        .args(["-rbundler", "-e", "'puts Bundler.bundle_path'"])
        .spawn()?
        .wait_with_output()
        .map(|out| out.stdout)?;
    String::from_utf8(bundle_path)
        .map_err(|_| Error::BadBundlePath)
        .map(Utf8PathBuf::from)
}

fn install_gems(downloaded: Vec<Downloaded>) -> Result<()> {
    // 1. Get the path where we want to put the gems from Bundler
    //    ruby -rbundler -e 'puts Bundler.bundle_path'
    let bundle_path = find_bundle_path()?;
    // 2. Unpack all the tarballs
    for download in downloaded {
        download.unpack_tarball(bundle_path.clone())?;
    }
    // 3. Generate binstubs into DIR/bin/
    // 4. Handle compiling native extensions for gems with native extensions
    // 5. Copy the .gem files and the .gemspec files into cache and specificatiosn?
    Ok(())
}

fn rv_http_client() -> Result<Client> {
    use reqwest::header;
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "X-RV-PLATFORM",
        header::HeaderValue::from_static(current_platform::CURRENT_PLATFORM),
    );
    headers.insert("X-RV-COMMAND", header::HeaderValue::from_static("ci"));

    let client = reqwest::Client::builder()
        .user_agent(format!("rv-{}", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        .build()?;

    Ok(client)
}

/// Downloads all gems from a Gemfile.lock
async fn download_gems<'i>(
    lockfile: GemfileDotLock<'i>,
    cache: &rv_cache::Cache,
    max_concurrent_requests: usize,
) -> Result<Vec<Downloaded<'i>>> {
    let all_sources = futures_util::stream::iter(lockfile.gem);
    let downloaded: Vec<_> = all_sources
        .map(|gem_source| download_gem_source(gem_source, cache, max_concurrent_requests))
        .buffered(10)
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
        .collect();
    Ok(downloaded)
}

struct Downloaded<'i> {
    contents: Bytes,
    spec: Spec<'i>,
}

impl<'i> Downloaded<'i> {
    fn unpack_tarball(self, bundle_path: Utf8PathBuf) -> Result<()> {
        // Unpack the tarball into DIR/gems/
        // It should contain a metadata zip, and a data zip
        // (and optionally, a checksum zip).
        let GemVersion { name, version } = self.spec.gem_version;
        let nameversion = format!("{name}-{version}");
        debug!("Unpacking {nameversion}");

        // Then unpack the tarball into it.
        let contents = std::io::Cursor::new(self.contents);
        let mut archive = tar::Archive::new(contents);
        for e in archive.entries()? {
            let entry = e?;
            let entry_path = entry.path()?;
            match entry_path.display().to_string().as_str() {
                "metadata.gz" => {
                    // Unzip the metadata file,
                    // then write it to
                    // BUNDLEPATH/specifications/name-version.gemspec

                    // First, create the destination.
                    let metadata_dir = bundle_path.join("specifications/");
                    std::fs::create_dir_all(&metadata_dir)?;
                    let filename = format!("{nameversion}.gemspec");
                    let dst_path = metadata_dir.join(filename);
                    let mut dst = std::fs::File::create(dst_path)?;

                    // Then write the (unzipped) source into the destination.
                    let mut unzipped_contents = GzDecoder::new(entry);
                    std::io::copy(&mut unzipped_contents, &mut dst)?;
                }
                "data.tar.gz" => {
                    // for every ENTRY in the data tar, unpack it to
                    // data.tar.gz => BUNDLEPATH/gems/name-version/ENTRY
                    let data_dir: std::path::PathBuf =
                        bundle_path.join("gems").join(&nameversion).into();
                    std::fs::create_dir_all(&data_dir)?;
                    let mut gem_data_archive = tar::Archive::new(GzDecoder::new(entry));
                    for e in gem_data_archive.entries()? {
                        let mut entry = e?;
                        let entry_path = entry.path()?;
                        let dst = data_dir.join(entry_path);

                        // Not sure if this is strictly necessary, or if we can know the
                        // intermediate directories ahead of time.
                        if let Some(dst_parent) = dst.parent() {
                            std::fs::create_dir_all(dst_parent)?;
                        }
                        entry.unpack(dst)?;
                    }
                }
                "checksums.yaml.gz" => {
                    // TODO: Validate these checksums
                }
                "data.tar.gz.sig" | "metadata.gz.sig" | "checksums.yaml.gz.sig" => {
                    // TODO: Validate these signatures.
                }
                other => {
                    info!("Unknown dir {other} in gem")
                }
            }
        }
        Ok(())
    }
}

fn url_for_spec(remote: &str, spec: &Spec<'_>) -> Result<Url> {
    let gem_name = spec.gem_version.name;
    let gem_version = spec.gem_version.version;
    let path = format!("gems/{gem_name}-{gem_version}.gem");
    let url = url::Url::parse(remote)
        .map_err(|err| Error::BadRemote {
            remote: remote.to_owned(),
            err,
        })?
        .join(&path)?;
    Ok(url)
}

/// Downloads all gems from a particular gem source,
/// e.g. from gems.coop or rubygems or something.
async fn download_gem_source<'i>(
    gem_source: GemSection<'i>,
    cache: &rv_cache::Cache,
    max_concurrent_requests: usize,
) -> Result<Vec<Downloaded<'i>>> {
    // TODO: If the gem server needs user credentials, accept them and add them to this client.
    let client = rv_http_client()?;

    // Get all URLs for downloading all gems from this source.

    // Download them all, concurrently.
    let spec_stream = futures_util::stream::iter(gem_source.specs);
    let downloaded_gems: Vec<_> = spec_stream
        .map(|spec| download_gem(gem_source.remote, spec, &client, cache))
        .buffered(max_concurrent_requests)
        .try_collect()
        .await?;
    Ok(downloaded_gems)
}

/// Download a single gem, from the given URL, using the given client.
async fn download_gem<'i>(
    remote: &str,
    spec: Spec<'i>,
    client: &Client,
    cache: &rv_cache::Cache,
) -> Result<Downloaded<'i>> {
    let url = url_for_spec(remote, &spec)?;
    let cache_key = rv_cache::cache_digest(url.as_ref());
    let cache_path = cache
        .shard(rv_cache::CacheBucket::Gem, "gems")
        .into_path_buf()
        .join(format!("{cache_key}.gem"));

    let contents;
    if cache_path.exists() {
        let data = tokio::fs::read(&cache_path).await?;
        contents = Bytes::from(data);
        // TODO: Validate checksum and download it again if mismatched.
        debug!("Reusing gem from {url} in cache");
    } else {
        contents = client.get(url.clone()).send().await?.bytes().await?;
        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&cache_path, &contents).await?;
        debug!("Downloaded gem from {url}");
    }
    // TODO: Validate the checksum from the Lockfile if present.
    Ok(Downloaded { contents, spec })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_download_gems() -> Result<()> {
        let file = "../rv-lockfile/tests/inputs/Gemfile.lock.test0".into();
        let cache = rv_cache::Cache::temp().unwrap();
        ci_inner(file, &cache, 10).await?;
        Ok(())
    }
}
