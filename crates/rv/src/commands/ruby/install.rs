use anstream::println;
use camino::{Utf8Path, Utf8PathBuf};
use core::panic;
use current_platform::CURRENT_PLATFORM;
use futures_util::StreamExt;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

use rv_ruby::{Release, request::RubyRequest};

use crate::{commands::ruby::list::fetch_available_rubies, config::Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    StripPrefixError(#[from] std::path::StripPrefixError),
    #[error("Major, minor, and patch version is required, but got {0}")]
    IncompleteVersion(RubyRequest),
    #[error("Download from URL {url} failed with status code {status}. Response body was {body}")]
    DownloadFailed {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("Could not get latest Ruby release")]
    GetLatestReleaseFailed { error: super::list::Error },
    #[error("Failed to unpack tarball path {0}")]
    InvalidTarballPath(PathBuf),
    #[error("rv does not (yet) support your platform ({0}). Sorry :(")]
    UnsupportedPlatform(&'static str),
}

type Result<T> = miette::Result<T, Error>;

pub async fn install(
    config: &Config,
    install_dir: Option<String>,
    requested: RubyRequest,
) -> Result<()> {
    let install_dir = match install_dir {
        Some(dir) => Utf8PathBuf::from(dir),
        None => match config.ruby_dirs.first() {
            Some(dir) => dir.clone(),
            None => panic!("No Ruby directories to install into"),
        },
    };

    if requested.patch.is_none() {
        Err(Error::IncompleteVersion(requested.clone()))?;
    }

    let latest: Release = fetch_available_rubies(&config.cache)
        .await
        .map_err(|error| Error::GetLatestReleaseFailed { error })?;
    let latest_release_tag = latest.name;
    let url = ruby_url(&requested.to_string(), &latest_release_tag)?;
    let tarball_path = tarball_path(config, &url);

    let new_dir = tarball_path.parent().unwrap();
    if !new_dir.exists() {
        std::fs::create_dir_all(new_dir)?;
    }

    if valid_tarball_exists(&tarball_path) {
        println!(
            "Tarball {} already exists, skipping download.",
            tarball_path.cyan()
        );
    } else {
        download_ruby_tarball(config, &url, &tarball_path).await?;
    }

    extract_ruby_tarball(&tarball_path, &install_dir, &requested.number())?;

    println!(
        "Installed Ruby version {} to {}",
        requested.to_string().cyan(),
        install_dir.cyan()
    );

    Ok(())
}

/// Does a usable tarball already exist at this path?
fn valid_tarball_exists(path: &Utf8Path) -> bool {
    let Ok(f) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(metadata) = f.metadata() else {
        return false;
    };
    if metadata.len() == 0 {
        return false;
    }
    true
}

fn ruby_url(version: &str, release_tag: &str) -> Result<String> {
    let version = version.strip_prefix("ruby-").unwrap();
    let arch = match CURRENT_PLATFORM {
        "aarch64-apple-darwin" => "arm64_sonoma",
        "x86_64-unknown-linux-gnu" => "x86_64_linux",
        "aarch64-unknown-linux-gnu" => "arm64_linux",
        _ => return Err(Error::UnsupportedPlatform(CURRENT_PLATFORM)),
    };

    let download_base = std::env::var("RV_RELEASES_URL")
        .unwrap_or("https://github.com/spinel-coop/rv-ruby/releases".to_owned());

    Ok(format!(
        "{}/download/{release_tag}/ruby-{version}.{arch}.tar.gz",
        download_base
    ))
}

fn tarball_path(config: &Config, url: impl AsRef<str>) -> Utf8PathBuf {
    let cache_key = rv_cache::cache_digest(url.as_ref());
    config
        .cache
        .shard(rv_cache::CacheBucket::Ruby, "tarballs")
        .into_path_buf()
        .join(format!("{cache_key}.tar.gz"))
}

fn temp_tarball_path(config: &Config, url: impl AsRef<str>) -> Utf8PathBuf {
    let cache_key = rv_cache::cache_digest(url.as_ref());
    config
        .cache
        .shard(rv_cache::CacheBucket::Ruby, "tarballs")
        .into_path_buf()
        .join(format!("{cache_key}.tar.gz.tmp"))
}

/// Write the file from this HTTP `response` to the given `path`.
/// While the stream is being handled, it'll be written to the given `temp_path`.
/// Then once the download finishes, the file will be renamed to `path`.
async fn write_to_filesystem(
    response: reqwest::Response,
    temp_path: &Utf8Path,
    path: &Utf8Path,
) -> Result<()> {
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }
    file.sync_all().await?;
    tokio::fs::rename(temp_path, path).await?;
    Ok(())
}

async fn download_ruby_tarball(
    config: &Config,
    url: &str,
    tarball_path: &Utf8PathBuf,
) -> Result<()> {
    // Start downloading the tarball.
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<error reading body: {e}>"));
        return Err(Error::DownloadFailed {
            url: url.to_string(),
            status,
            body,
        });
    }

    // Write the tarball bytes to the filesystem.
    let temp_path = temp_tarball_path(config, url);
    if let Err(e) = write_to_filesystem(response, &temp_path, tarball_path).await {
        // Clean up the temporary file if there was any error.
        tokio::fs::remove_file(temp_path).await?;
        return Err(e);
    }

    println!("Downloaded {} to {}", url.cyan(), tarball_path.cyan());
    Ok(())
}

fn extract_ruby_tarball(
    tarball_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    version: &str,
) -> Result<()> {
    // dbg!(&tarball_path);
    std::fs::create_dir_all(format!("{rubies_dir}/{version}"))?;
    let tarball = std::fs::File::open(tarball_path)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    for e in archive.entries()? {
        let mut entry = e?;
        let entry_path = entry.path()?;
        let path = entry_path
            .to_str()
            .ok_or_else(|| Error::InvalidTarballPath(entry_path.to_path_buf()))?
            .replace(
                &format!("rv-ruby@{version}/{version}"),
                &format!("ruby-{version}"),
            )
            .replace('@', "-");
        let dst = rubies_dir.join(path);
        entry.unpack(dst)?;
    }

    Ok(())
}
