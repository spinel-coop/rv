use anstream::println;
use bytesize::ByteSize;
use camino::{Utf8Path, Utf8PathBuf};
use core::panic;
use futures_util::StreamExt;
use indicatif::ProgressStyle;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info_span};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use rv_gem_types::Platform;
use rv_ruby::{request::RubyRequest, version::RubyVersion};

use crate::config::Config;
use crate::progress::WorkProgress;

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
    #[error(transparent)]
    UnsupportedPlatform(#[from] rv_gem_types::platform::PlatformError),
}

type Result<T> = miette::Result<T, Error>;

pub async fn install(
    config: &Config,
    install_dir: Option<String>,
    requested: Option<RubyRequest>,
    tarball_path: Option<String>,
) -> Result<()> {
    let progress = WorkProgress::new();

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
            .find_match_in(&remote_rubies)
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
            download_and_extract_remote_tarball(
                config,
                &install_dir,
                &selected_version.number(),
                &progress,
            )
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
    progress: &WorkProgress,
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
        download_ruby_tarball(config, &url, &tarball_path, version, progress).await?;
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
    let arch = Platform::local_precompiled_ruby_arch()?;

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
    total_size: u64,
    progress: &WorkProgress,
    span: &tracing::Span,
) -> Result<()> {
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let chunk_len = chunk.len() as u64;
        file.write_all(&chunk).await?;

        downloaded += chunk_len;
        progress.complete_many(chunk_len);

        // Update the progress message
        if total_size > 0 {
            span.pb_set_message(&format!(
                "({} / {})",
                ByteSize(downloaded),
                ByteSize(total_size)
            ));
        } else {
            span.pb_set_message(&format!("({})", ByteSize(downloaded)));
        }
    }
    file.sync_all().await?;
    tokio::fs::rename(temp_path, path).await?;
    Ok(())
}

async fn download_ruby_tarball(
    config: &Config,
    url: &str,
    tarball_path: &Utf8PathBuf,
    version: &str,
    progress: &WorkProgress,
) -> Result<()> {
    debug!("Downloading tarball from {url}");
    // Build the request with optional GitHub authentication
    let client = reqwest::Client::new();
    let mut request_builder = client.get(url);

    // Add GitHub token authentication if available and URL is from GitHub
    // Check GITHUB_TOKEN first (GitHub Actions), then GH_TOKEN (GitHub CLI/general use)
    if crate::config::github::is_github_url(url) {
        if let Some(token) = crate::config::github::github_token() {
            debug!("Using authenticated GitHub request for tarball download");
            request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
        } else {
            debug!("No GitHub token found, using unauthenticated request for tarball download");
        }
    }
    // Start downloading the tarball.
    let response = request_builder.send().await?;
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

    // Get Content-Length for progress tracking
    let total_size = response.content_length().unwrap_or(0);

    // Set up progress tracking
    progress.start_phase(total_size, 100);

    let span = info_span!("Downloading Ruby", version = version);
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name} {msg}").unwrap());
    let _guard = span.enter();

    // Write the tarball bytes to the filesystem.
    let temp_path = temp_tarball_path(config, url);
    if let Err(e) = write_to_filesystem(
        response,
        &temp_path,
        tarball_path,
        total_size,
        progress,
        &span,
    )
    .await
    {
        // Clean up the temporary file if there was any error.
        tokio::fs::remove_file(temp_path).await?;
        return Err(e);
    }

    Ok(())
}

fn extract_ruby_tarball(
    tarball_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    version: &str,
) -> Result<()> {
    let span = info_span!("Installing Ruby", version = version);
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name}").unwrap());
    let _guard = span.enter();

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
