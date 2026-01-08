use anstream::println;
use camino::{Utf8Path, Utf8PathBuf};
use core::panic;
use current_platform::CURRENT_PLATFORM;
use futures_util::StreamExt;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tracing::debug;

use rv_ruby::{request::RubyRequest, version::RubyVersion};

use crate::config::Config;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    StripPrefixError(#[from] std::path::StripPrefixError),
    #[error("no matching ruby version found")]
    NoMatchingRuby,
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
    requested: Option<RubyRequest>,
    tarball_path: Option<String>,
) -> Result<()> {
    let requested_range = match requested {
        None => config.ruby_request(),
        Some(version) => version,
    };

    let selected_version = if let Ok(version) = RubyVersion::try_from(requested_range.clone()) {
        debug!(
            "Skipping the rv-ruby releases fetch because the user has given a specific ruby version {version}"
        );
        version
    } else {
        debug!("Fetching available rubies, because user gave an underspecified Ruby range");
        let remote_rubies = config.remote_rubies().await;
        requested_range
            .find_match_in(remote_rubies)
            .ok_or(Error::NoMatchingRuby)?
            .version
    };

    let install_dir = match install_dir {
        Some(dir) => Utf8PathBuf::from(dir),
        None => match config.ruby_dirs.first() {
            Some(dir) => dir.clone(),
            None => panic!("No Ruby directories to install into"),
        },
    };

    match tarball_path {
        Some(tarball_path) => {
            extract_local_ruby_tarball(tarball_path, &install_dir, &selected_version.number())
                .await?
        }
        None => {
            download_and_extract_remote_tarball(config, &install_dir, &selected_version.number())
                .await?
        }
    }

    println!(
        "Installed Ruby version {} to {}",
        selected_version.to_string().cyan(),
        install_dir.cyan()
    );

    Ok(())
}

// downloads and extracts a remote ruby tarball
async fn download_and_extract_remote_tarball(
    config: &Config,
    install_dir: &Utf8PathBuf,
    version: &str,
) -> Result<()> {
    let url = ruby_url(version)?;
    let tarball_path = tarball_path(config, &url);

    let new_dir = tarball_path.parent().unwrap();
    if !new_dir.exists() {
        fs_err::create_dir_all(new_dir)?;
    }

    if valid_tarball_exists(&tarball_path) {
        println!(
            "Tarball {} already exists, skipping download.",
            tarball_path.cyan()
        );
    } else {
        download_ruby_tarball(config, &url, &tarball_path).await?;
    }

    extract_ruby_tarball(&tarball_path, install_dir, version)?;

    Ok(())
}

// extract a local ruby tarball
async fn extract_local_ruby_tarball(
    tarball_path: String,
    install_dir: &Utf8PathBuf,
    version: &str,
) -> Result<()> {
    extract_ruby_tarball(Utf8Path::new(&tarball_path), install_dir, version)?;

    Ok(())
}

/// Does a usable tarball already exist at this path?
fn valid_tarball_exists(path: &Utf8Path) -> bool {
    fs_err::metadata(path).is_ok_and(|m| m.is_file() && m.len() > 0)
}

fn ruby_url(version: &str) -> Result<String> {
    let arch = match CURRENT_PLATFORM {
        "aarch64-apple-darwin" => "arm64_sonoma",
        "x86_64-apple-darwin" => "ventura",
        "x86_64-unknown-linux-gnu" => "x86_64_linux",
        "aarch64-unknown-linux-gnu" => "arm64_linux",
        other => return Err(Error::UnsupportedPlatform(other)),
    };

    let download_base = std::env::var("RV_INSTALL_URL")
        .unwrap_or("https://github.com/spinel-coop/rv-ruby/releases/latest/download".to_owned());

    Ok(format!("{download_base}/ruby-{version}.{arch}.tar.gz"))
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
    if !rubies_dir.exists() {
        fs_err::create_dir_all(rubies_dir)?;
    }
    let tarball = fs_err::File::open(tarball_path)?;
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
