use anstream::println;
use camino::{Utf8Path, Utf8PathBuf};
use core::panic;
use current_platform::CURRENT_PLATFORM;
use futures_util::StreamExt;
use owo_colors::OwoColorize;
use std::path::PathBuf;

use rv_ruby::request::RubyRequest;

use crate::config::Config;

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
    #[error("Download from URL {0} failed with status code {1}")]
    DownloadFailed(String, reqwest::StatusCode),
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
    let url = ruby_url(&requested.to_string())?;
    let tarball_path = tarball_path(config, &url);

    if !tarball_path.parent().unwrap().exists() {
        std::fs::create_dir_all(tarball_path.parent().unwrap())?;
    }

    if tarball_path.exists() {
        println!(
            "Tarball {} already exists, skipping download.",
            tarball_path.cyan()
        );
    } else {
        download_ruby_tarball(&url, &tarball_path).await?;
    }

    extract_ruby_tarball(&tarball_path, &install_dir).await?;

    println!(
        "Installed Ruby version {} to {}",
        requested.to_string().cyan(),
        install_dir.cyan()
    );

    Ok(())
}

fn ruby_url(version: &str) -> Result<String> {
    let number = version.strip_prefix("ruby-").unwrap_or(version);
    let arch = match CURRENT_PLATFORM {
        "aarch64-apple-darwin" => "arm64_sonoma",
        "x86_64-unknown-linux-gnu" => "x86_64_linux",
        "aarch64-unknown-linux-gnu" => "arm64_linux",
        _ => return Err(Error::UnsupportedPlatform(CURRENT_PLATFORM)),
    };

    Ok(format!(
        "https://github.com/spinel-coop/rv-ruby/releases/download/{number}/portable-{version}.{arch}.bottle.tar.gz"
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

async fn download_ruby_tarball(url: &str, tarball_path: &Utf8PathBuf) -> Result<()> {
    let mut file = tokio::fs::File::create(tarball_path).await?;

    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(Error::DownloadFailed(url.to_string(), response.status()));
    }

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;
    }

    println!("Downloaded {} to {}", url.cyan(), tarball_path.cyan());
    Ok(())
}

async fn extract_ruby_tarball(tarball_path: &Utf8Path, dir: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let tarball = std::fs::File::open(tarball_path)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    for e in archive.entries()? {
        let mut entry = e?;
        let entry_path = entry.path()?;
        let path = entry_path
            .to_str()
            .ok_or_else(|| Error::InvalidTarballPath(entry_path.to_path_buf()))?
            .replace("portable-ruby/", "ruby-");
        let entry_path = dir.join(path);
        entry.unpack(entry_path)?;
    }

    Ok(())
}
