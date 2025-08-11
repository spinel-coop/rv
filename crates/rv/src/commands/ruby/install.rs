use camino::{Utf8Path, Utf8PathBuf};
use core::panic;
use miette::Diagnostic;
use owo_colors::OwoColorize;
use std::path::PathBuf;

use rv_dirs::user_cache_dir;
use rv_ruby::version_request::VersionRequest;

use crate::config::Config;

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    StripPrefixError(#[from] std::path::StripPrefixError),
    #[error("Major, minor, and patch version is required, but got {0}")]
    IncompleteVersion(VersionRequest),
    #[error("Download from URL {0} failed with status code {1}")]
    DownloadFailed(String, reqwest::StatusCode),
    #[error("Failed to unpack tarball path {0}")]
    InvalidTarballPath(PathBuf),
}

type Result<T> = miette::Result<T, Error>;

pub async fn install(
    config: &Config,
    install_dir: Option<String>,
    requested: VersionRequest,
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
    let url = ruby_url(&requested.to_string());
    let tarball_path = tarball_path(config, &requested.to_string());

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

fn ruby_url(version: &str) -> String {
    format!(
        "https://github.com/spinel-coop/rv-ruby/releases/download/{version}/{version}.arm64_sonoma.bottle.tar.gz"
    )
}

fn tarball_path(config: &Config, version: &str) -> Utf8PathBuf {
    user_cache_dir(&config.root).join(format!("rubies/{version}.tar.gz"))
}

async fn download_ruby_tarball(url: &str, tarball_path: &Utf8PathBuf) -> Result<()> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(Error::DownloadFailed(url.to_string(), response.status()));
    }
    let tarball = response.bytes().await?;
    // write tarball to tarball_path
    std::fs::write(tarball_path, &tarball)?;

    println!("Downloaded {} to {}", url.cyan(), tarball_path.cyan());

    Ok(())
}

async fn extract_ruby_tarball(tarball_path: &Utf8Path, dir: &Utf8Path) -> Result<()> {
    let tarball = std::fs::File::open(tarball_path)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    for e in archive.entries()? {
        let mut entry = e?;
        let entry_path = entry.path()?;
        let path = entry_path.strip_prefix("portable-ruby/")?;
        let path = path
            .to_str()
            .ok_or_else(|| Error::InvalidTarballPath(entry_path.to_path_buf()))?;
        let entry_path = dir.join(path);
        entry.unpack(entry_path)?;
    }

    Ok(())
}
