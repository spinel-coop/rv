use anstream::println;
use bytesize::ByteSize;
use camino::{Utf8Path, Utf8PathBuf};
use core::panic;
use futures_util::StreamExt;
use indicatif::ProgressStyle;
use owo_colors::OwoColorize;
use reqwest::StatusCode;
use rv_ruby::version::RubyVersion;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info_span};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use rv_platform::HostPlatform;
use rv_ruby::request::RubyRequest;

use crate::progress::WorkProgress;
use crate::{GlobalArgs, config::Config};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    StripPrefixError(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    ZipError(#[from] zip::result::ZipError),
    #[error(transparent)]
    SevenZipError(#[from] sevenz_rust2::Error),
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
    #[error("Failed to unpack archive path {0}")]
    InvalidTarballPath(PathBuf),
    #[error(transparent)]
    UnsupportedPlatform(#[from] rv_platform::UnsupportedPlatformError),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) async fn install(
    global_args: &GlobalArgs,
    install_dir: Option<String>,
    request: Option<RubyRequest>,
    tarball_path: Option<Utf8PathBuf>,
) -> Result<()> {
    let config = &Config::new(global_args, request)?;

    let progress = WorkProgress::new();

    let requested_range = config.ruby_request();

    let version = if let Ok(version) = RubyVersion::try_from(requested_range.clone()) {
        debug!(
            "Skipping the rv-ruby releases fetch because the user has given a specific ruby version {version}"
        );
        version
    } else {
        debug!("Fetching available rubies, because user gave an underspecified Ruby range");
        let remote_rubies = config.remote_rubies().await;
        let selected = requested_range
            .find_match_in(&remote_rubies)
            .ok_or(Error::NoMatchingRuby)?
            .version;
        RubyVersion::Released(selected)
    };

    let install_dir = match install_dir {
        Some(dir) => Utf8PathBuf::from(dir),
        None => match config.ruby_dirs.first() {
            Some(dir) => dir.clone(),
            None => panic!("No Ruby directories to install into"),
        },
    };

    let archive_path = if let Some(path) = tarball_path {
        path
    } else {
        download_tarball(config, &version, &progress).await?
    };

    extract_ruby_archive(&archive_path, &install_dir, &version)?;

    println!(
        "Installed Ruby version {} to {}",
        version.to_string().cyan(),
        install_dir.cyan()
    );

    Ok(())
}

// downloads and extracts a remote ruby archive (tarball or zip)
async fn download_tarball(
    config: &Config,
    version: &RubyVersion,
    progress: &WorkProgress,
) -> Result<Utf8PathBuf> {
    let host = HostPlatform::current()?;
    let url = ruby_url(version, &host);
    let archive_path = archive_cache_path(config, &url, &host);

    let cache_dir = archive_path.parent().unwrap();
    if !cache_dir.exists() {
        fs_err::create_dir_all(cache_dir)?;
    }

    if valid_archive_exists(&archive_path) {
        println!(
            "Archive {} already exists, skipping download.",
            archive_path.cyan()
        );
    } else {
        download_ruby_archive(config, &url, &archive_path, version, progress, &host).await?;
    }

    Ok(archive_path)
}

/// Does a usable archive already exist at this path?
fn valid_archive_exists(path: &Utf8Path) -> bool {
    fs_err::metadata(path).is_ok_and(|m| m.is_file() && m.len() > 0)
}

fn ruby_url(version: &RubyVersion, host: &HostPlatform) -> String {
    let arch = host.ruby_arch_str();
    let ext = host.archive_ext();
    let number = version.number();

    // Windows uses RubyInstaller2 directly
    if host.is_windows() {
        let download_base = std::env::var("RV_INSTALL_URL").unwrap_or_else(|_| {
            format!(
                "https://github.com/oneclick/rubyinstaller2/releases/download/RubyInstaller-{number}-1"
            )
        });
        // RubyInstaller2 URL pattern: rubyinstaller-{version}-1-x64.7z
        return format!("{download_base}/rubyinstaller-{number}-1-{arch}.{ext}");
    }

    // macOS/Linux use rv-ruby
    let (download_base, version_str) = std::env::var("RV_INSTALL_URL")
        .map(|var| (var, version.to_string()))
        .unwrap_or_else(|_| match version {
            RubyVersion::Dev => (
                "https://github.com/spinel-coop/rv-ruby-dev/releases/latest/download".to_owned(),
                version.to_string(),
            ),
            RubyVersion::Released(_) => (
                "https://github.com/spinel-coop/rv-ruby/releases/latest/download".to_owned(),
                version.to_string(),
            ),
        });

    format!("{download_base}/{version_str}.{arch}.{ext}")
}

fn archive_cache_path(config: &Config, url: impl AsRef<str>, host: &HostPlatform) -> Utf8PathBuf {
    let ext = host.archive_ext();
    let cache_key = rv_cache::cache_digest(url.as_ref());
    config
        .cache
        .shard(rv_cache::CacheBucket::Ruby, "tarballs")
        .into_path_buf()
        .join(format!("{cache_key}.{ext}"))
}

fn temp_archive_path(config: &Config, url: impl AsRef<str>, host: &HostPlatform) -> Utf8PathBuf {
    let ext = host.archive_ext();
    let cache_key = rv_cache::cache_digest(url.as_ref());
    config
        .cache
        .shard(rv_cache::CacheBucket::Ruby, "tarballs")
        .into_path_buf()
        .join(format!("{cache_key}.{ext}.tmp"))
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

async fn download_ruby_archive(
    config: &Config,
    url: &str,
    archive_path: &Utf8PathBuf,
    version: &RubyVersion,
    progress: &WorkProgress,
    host: &HostPlatform,
) -> Result<()> {
    debug!("Downloading archive from {url}");
    // Build the request with optional GitHub authentication
    let client = reqwest::Client::new();
    let mut request_builder = client.get(url);

    // Add GitHub token authentication if available and URL is from GitHub
    // Check GITHUB_TOKEN first (GitHub Actions), then GH_TOKEN (GitHub CLI/general use)
    if crate::config::github::is_github_url(url) {
        if let Some(token) = crate::config::github::github_token() {
            debug!("Using authenticated GitHub request for archive download");
            request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
        } else {
            debug!("No GitHub token found, using unauthenticated request for archive download");
        }
    }
    // Start downloading the archive.
    let response = request_builder.send().await?;
    if !response.status().is_success() {
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            return Err(Error::NoMatchingRuby);
        }
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

    let span = info_span!("Downloading Ruby", version = version.number());
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name} {msg}").unwrap());
    let _guard = span.enter();

    // Write the archive bytes to the filesystem.
    let temp_path = temp_archive_path(config, url, host);
    if let Err(e) = write_to_filesystem(
        response,
        &temp_path,
        archive_path,
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

fn extract_ruby_archive(
    archive_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    version: &RubyVersion,
) -> Result<()> {
    let host = HostPlatform::current()?;
    let span = info_span!("Installing Ruby", version = version.number());
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name}").unwrap());
    let _guard = span.enter();

    if !rubies_dir.exists() {
        fs_err::create_dir_all(rubies_dir)?;
    }

    // Determine archive type by extension
    let extension = archive_path.extension().unwrap_or("");
    match extension {
        "zip" => extract_zip(archive_path, rubies_dir, version),
        "7z" => extract_7z(archive_path, rubies_dir, version, &host),
        _ => extract_tarball(archive_path, rubies_dir, version),
    }
}

fn extract_tarball(
    tarball_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    version: &RubyVersion,
) -> Result<()> {
    let tarball = fs_err::File::open(tarball_path)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    for e in archive.entries()? {
        let mut entry = e?;
        let entry_path = entry.path()?;

        let dst: PathBuf = if version.is_dev() {
            // Strip the first two path components
            let mut path = entry_path.components();
            path.next();
            path.next();

            rubies_dir
                .as_std_path()
                .join(format!("ruby-{}", version.number()))
                .join(path.as_path())
        } else {
            let version_number = version.number();
            let to_replace = format!("rv-ruby@{}/{}", version_number, version_number);
            let path = entry_path
                .to_str()
                .ok_or_else(|| Error::InvalidTarballPath(entry_path.to_path_buf()))?
                .replace(&to_replace, &format!("ruby-{}", version_number))
                .replace('@', "-");
            rubies_dir.join(path).into()
        };

        entry.unpack(dst)?;
    }
    Ok(())
}

fn extract_zip(zip_path: &Utf8Path, rubies_dir: &Utf8Path, version: &RubyVersion) -> Result<()> {
    let file = fs_err::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_path = entry.name().to_string();

        // Normalize path: repackage RubyInstaller format to rv format
        let path = entry_path
            .replace(
                &format!("rubyinstaller-{}", version.number()),
                &format!("ruby-{}", version.number()),
            )
            .replace('\\', "/"); // Normalize Windows path separators

        let dst = rubies_dir.join(&path);

        if entry.is_dir() {
            fs_err::create_dir_all(&dst)?;
        } else {
            if let Some(parent) = dst.parent() {
                fs_err::create_dir_all(parent)?;
            }
            let mut outfile = fs_err::File::create(&dst)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }
    Ok(())
}

fn extract_7z(
    archive_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    version: &RubyVersion,
    host: &HostPlatform,
) -> Result<()> {
    // Extract 7z archive to rubies_dir
    sevenz_rust2::decompress_file(archive_path.as_std_path(), rubies_dir.as_std_path())?;

    // RubyInstaller2 extracts to: rubyinstaller-{version}-1-{arch}/
    // We need to rename it to: ruby-{version}/
    let arch = host.ruby_arch_str();
    let extracted_dir = rubies_dir.join(format!("rubyinstaller-{}-1-{arch}", version.number()));
    let target_dir = rubies_dir.join(format!("ruby-{}", version.number()));

    if extracted_dir.exists() {
        fs_err::rename(&extracted_dir, &target_dir)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use assert_fs::prelude::*;
    use std::io::Write as _;
    use std::str::FromStr;

    #[track_caller]
    fn v(version: &str) -> RubyVersion {
        RubyVersion::from_str(version).unwrap()
    }

    #[test]
    fn test_ruby_url_unix() {
        let host = HostPlatform::from_target_triple("aarch64-apple-darwin").unwrap();
        let url = ruby_url(&v("3.4.1"), &host);

        assert_eq!(
            url,
            "https://github.com/spinel-coop/rv-ruby/releases/latest/download/ruby-3.4.1.arm64_sonoma.tar.gz"
        );
    }

    #[test]
    fn test_ruby_url_windows() {
        let host = HostPlatform::from_target_triple("x86_64-pc-windows-msvc").unwrap();
        let url = ruby_url(&v("3.4.1"), &host);

        assert_eq!(
            url,
            "https://github.com/oneclick/rubyinstaller2/releases/download/RubyInstaller-3.4.1-1/rubyinstaller-3.4.1-1-x64.7z"
        );
    }

    #[test]
    fn test_ruby_url_unix_dev() {
        let host = HostPlatform::from_target_triple("aarch64-apple-darwin").unwrap();
        let url = ruby_url(&v("dev"), &host);

        assert_eq!(
            url,
            "https://github.com/spinel-coop/rv-ruby-dev/releases/latest/download/ruby-dev.arm64_sonoma.tar.gz"
        );
    }
    #[test]
    fn test_extract_zip_creates_correct_structure() {
        let temp_dir = TempDir::new().unwrap();
        let rubies_dir = temp_dir.child("rubies");
        rubies_dir.create_dir_all().unwrap();

        let zip_path = temp_dir.child("test-ruby.zip");
        {
            let file = std::fs::File::create(zip_path.path()).unwrap();
            let mut zip = zip::ZipWriter::new(file);

            let options: zip::write::SimpleFileOptions = Default::default();
            zip.add_directory::<_, ()>("rubyinstaller-3.4.1/", options)
                .unwrap();
            zip.add_directory::<_, ()>("rubyinstaller-3.4.1/bin/", options)
                .unwrap();

            zip.start_file("rubyinstaller-3.4.1/bin/ruby.exe", options)
                .unwrap();
            zip.write_all(b"fake ruby executable").unwrap();

            zip.finish().unwrap();
        }

        let rubies_path = Utf8Path::from_path(rubies_dir.path()).unwrap();
        let zip_utf8_path = Utf8Path::from_path(zip_path.path()).unwrap();
        let version = RubyVersion::Released("3.4.1".parse().unwrap());
        extract_zip(zip_utf8_path, rubies_path, &version).unwrap();

        let ruby_dir = rubies_dir.child("ruby-3.4.1");
        assert!(ruby_dir.exists(), "ruby-3.4.1 directory should exist");

        let bin_dir = ruby_dir.child("bin");
        assert!(bin_dir.exists(), "bin directory should exist");

        let ruby_exe = bin_dir.child("ruby.exe");
        assert!(ruby_exe.exists(), "ruby.exe should exist");

        let content = std::fs::read_to_string(ruby_exe.path()).unwrap();
        assert_eq!(content, "fake ruby executable");
    }

    #[test]
    fn test_extract_ruby_archive_delegates_to_zip_extractor() {
        let temp_dir = TempDir::new().unwrap();
        let rubies_dir = temp_dir.child("rubies");
        rubies_dir.create_dir_all().unwrap();

        let zip_path = temp_dir.child("test.zip");
        {
            let file = std::fs::File::create(zip_path.path()).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::SimpleFileOptions = Default::default();
            zip.add_directory::<_, ()>("rubyinstaller-3.4.1/", options)
                .unwrap();
            zip.finish().unwrap();
        }

        let rubies_path = Utf8Path::from_path(rubies_dir.path()).unwrap();
        let zip_utf8_path = Utf8Path::from_path(zip_path.path()).unwrap();

        let result = extract_ruby_archive(zip_utf8_path, rubies_path, &v("3.4.1"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_archive_exists_returns_false_for_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let missing = temp_dir.child("missing.tar.gz");
        assert!(!valid_archive_exists(
            Utf8Path::from_path(missing.path()).unwrap()
        ));
    }

    #[test]
    fn test_valid_archive_exists_returns_false_for_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let empty = temp_dir.child("empty.tar.gz");
        empty.touch().unwrap();
        assert!(!valid_archive_exists(
            Utf8Path::from_path(empty.path()).unwrap()
        ));
    }

    #[test]
    fn test_valid_archive_exists_returns_true_for_file_with_content() {
        let temp_dir = TempDir::new().unwrap();
        let valid = temp_dir.child("valid.tar.gz");
        valid.write_binary(b"some content").unwrap();
        assert!(valid_archive_exists(
            Utf8Path::from_path(valid.path()).unwrap()
        ));
    }
}
