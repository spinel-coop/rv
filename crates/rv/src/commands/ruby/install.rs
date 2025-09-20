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
    let base_url = std::env::var("RV_RELEASES_URL").unwrap_or_else(|_| "https://github.com/spinel-coop/rv-ruby/releases".to_string());
    let url = ruby_url(&base_url, &requested.to_string());
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
        download_ruby_tarball(config, &url, &tarball_path).await?;
    }

    extract_ruby_tarball(&tarball_path, &install_dir).await?;

    println!(
        "Installed Ruby version {} to {}",
        requested.to_string().cyan(),
        install_dir.cyan()
    );

    Ok(())
}

fn ruby_url(base_url: &str, version: &str) -> String {
    let number = version.strip_prefix("ruby-").unwrap_or(version);
    let arch = match CURRENT_PLATFORM {
        "aarch64-apple-darwin" => "arm64_sonoma",
        "x86_64-unknown-linux-gnu" => "x86_64_linux",
        "aarch64-unknown-linux-gnu" => "arm64_linux",
        _ => panic!("rv does not (yet) support {}. Sorry :(", CURRENT_PLATFORM),
    };

    // Handle both GitHub API URL and direct download URL formats
    let download_base = if base_url.contains("api.github.com") {
        "https://github.com/spinel-coop/rv-ruby/releases"
    } else {
        base_url
    };

    format!(
        "{}/download/{number}/portable-{version}.{arch}.bottle.tar.gz",
        download_base
    )
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

async fn download_ruby_tarball(
    config: &Config,
    url: &str,
    tarball_path: &Utf8PathBuf,
) -> Result<()> {
    let temp_path = temp_tarball_path(config, url);

    let mut file = tokio::fs::File::create(&temp_path).await?;

    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(Error::DownloadFailed(url.to_string(), response.status()));
    }

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.inspect_err(|_| {
            let temp_path = temp_path.clone();
            tokio::spawn(async move {
                let _ = tokio::fs::remove_file(temp_path).await;
            });
        })?;
        tokio::io::copy(&mut chunk.as_ref(), &mut file)
            .await
            .inspect_err(|_| {
                let temp_path = temp_path.clone();
                tokio::spawn(async move {
                    let _ = tokio::fs::remove_file(temp_path).await;
                });
            })?;
    }

    file.sync_all().await?;
    drop(file);

    tokio::fs::rename(&temp_path, tarball_path)
        .await
        .inspect_err(|_| {
            let temp_path = temp_path.clone();
            tokio::spawn(async move {
                let _ = tokio::fs::remove_file(temp_path).await;
            });
        })?;

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
