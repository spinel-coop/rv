use bytes::Bytes;
use camino::Utf8PathBuf;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use reqwest::Client;
use rv_lockfile::datatypes::GemfileDotLock;
use url::Url;

use crate::config::Config;
use std::io;

const CONCURRENT_REQUESTS_PER_SOURCE: usize = 10;

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
}

type Result<T> = std::result::Result<T, Error>;

pub async fn ci(config: &Config) -> Result<()> {
    let lockfile_path;
    if let Some(path) = &config.gemfile {
        lockfile_path = format!("{}.lock", path.clone()).into();
    } else {
        lockfile_path = "Gemfile.lock".into();
    }
    ci_inner(lockfile_path, &config.cache).await
}

async fn ci_inner(lockfile_path: Utf8PathBuf, cache: &rv_cache::Cache) -> Result<()> {
    let lockfile_contents = std::fs::read_to_string(lockfile_path)?;
    let lockfile = rv_lockfile::parse(&lockfile_contents)?;
    download_gems(lockfile, cache).await?;
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

async fn download_gems<'i>(lockfile: GemfileDotLock<'i>, cache: &rv_cache::Cache) -> Result<()> {
    let client = rv_http_client()?;
    for gem_source in lockfile.gem {
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
        let url_stream = futures_util::stream::iter(urls);
        let _downloaded_gems: Vec<(Bytes, Url)> = url_stream
            .map(|url| download_gem(url, &client, &cache))
            .buffered(CONCURRENT_REQUESTS_PER_SOURCE)
            .try_collect()
            .await?;
    }
    Ok(())
}

async fn download_gem(url: Url, client: &Client, cache: &rv_cache::Cache) -> Result<(Bytes, Url)> {
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
    Ok((contents, url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_download_gems() -> Result<()> {
        eprintln!("{:?}", std::env::current_dir());
        let file = "../rv-lockfile/tests/inputs/Gemfile.lock.test0".into();
        let cache = rv_cache::Cache::temp().unwrap();
        ci_inner(file, &cache).await?;
        Ok(())
    }
}
