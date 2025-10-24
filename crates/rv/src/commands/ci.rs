use bytes::Bytes;
use camino::Utf8PathBuf;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use reqwest::Client;
use rv_lockfile::datatypes::GemSection;
use rv_lockfile::datatypes::GemfileDotLock;
use url::Url;

use crate::config::Config;
use std::io;

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

fn install_gems(gems: Vec<Vec<Downloaded>>) -> Result<()> {
    // 1. Get the path where we want to put the gems from Bundler
    //    ruby -rbundler -e 'puts Bundler.bundle_path'
    let bundle_path = find_bundle_path()?;
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
) -> Result<Vec<Vec<Downloaded>>> {
    let all_sources = futures_util::stream::iter(lockfile.gem);
    let downloaded: Vec<_> = all_sources
        .map(|gem_source| download_gem_source(gem_source, cache, max_concurrent_requests))
        .buffered(10)
        .try_collect()
        .await?;
    Ok(downloaded)
}

struct Downloaded {
    contents: Bytes,
    from: Url,
}

/// Downloads all gems from a particular gem source,
/// e.g. from gems.coop or rubygems or something.
async fn download_gem_source<'i>(
    gem_source: GemSection<'i>,
    cache: &rv_cache::Cache,
    max_concurrent_requests: usize,
) -> Result<Vec<Downloaded>> {
    // TODO: If the gem server needs user credentials, accept them and add them to this client.
    let client = rv_http_client()?;

    // Get all URLs for downloading all gems from this source.
    let urls = gem_source
        .specs
        .iter()
        .map(|gem| {
            let remote = gem_source.remote;
            let gem_name = gem.gem_version.name;
            let gem_version = gem.gem_version.version;
            let path = format!("gems/{gem_name}-{gem_version}.gem");
            let url = url::Url::parse(remote)
                .map_err(|err| Error::BadRemote {
                    remote: remote.to_owned(),
                    err,
                })?
                .join(&path)?;
            Ok(url)
        })
        .collect::<Result<Vec<Url>>>()?;

    // Download them all, concurrently.
    let url_stream = futures_util::stream::iter(urls);
    let downloaded_gems: Vec<_> = url_stream
        .map(|url| download_gem(url, &client, cache))
        .buffered(max_concurrent_requests)
        .try_collect()
        .await?;
    Ok(downloaded_gems)
}

/// Download a single gem, from the given URL, using the given client.
async fn download_gem(url: Url, client: &Client, cache: &rv_cache::Cache) -> Result<Downloaded> {
    eprintln!("Downloading from {url}");
    let cache_key = rv_cache::cache_digest(url.as_ref());
    let cache_path = cache
        .shard(rv_cache::CacheBucket::Gem, "gems")
        .into_path_buf()
        .join(format!("{cache_key}.gem"));

    let contents;
    if cache_path.exists() {
        let data = tokio::fs::read(&cache_path).await?;
        contents = Bytes::from(data);
    } else {
        contents = client.get(url.clone()).send().await?.bytes().await?;
        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&cache_path, &contents).await?;
    }
    // TODO: Validate the checksum from the Lockfile if present.
    Ok(Downloaded {
        contents,
        from: url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_download_gems() -> Result<()> {
        eprintln!("{:?}", std::env::current_dir());
        let file = "../rv-lockfile/tests/inputs/Gemfile.lock.test0".into();
        let cache = rv_cache::Cache::temp().unwrap();
        ci_inner(file, &cache, 10).await?;
        Ok(())
    }
}
