use anstream::println;
use bytesize::ByteSize;
use camino::{Utf8Path, Utf8PathBuf};
use core::panic;
use futures_util::StreamExt;
use indicatif::ProgressStyle;
use owo_colors::OwoColorize;
use reqwest::StatusCode;
use std::borrow::Cow;
use std::io::Read as _;
use std::path::{Component, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info_span, warn};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use rv_platform::HostPlatform;
use rv_ruby::engine::RubyEngine;
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
    #[error("Could not get latest ruby-dev release")]
    GetLatestDevReleaseFailed,
    #[error("Paths including .. are not allowed inside archives, but found {0}")]
    DirectoryTraversalError(String),
    #[error(transparent)]
    UnsupportedPlatform(#[from] rv_platform::UnsupportedPlatformError),
}

type Result<T> = miette::Result<T, Error>;

pub(crate) async fn install(
    global_args: &GlobalArgs,
    install_dir: Option<String>,
    request: Option<RubyRequest>,
    tarball_path: Option<Utf8PathBuf>,
    force: bool,
) -> Result<()> {
    let config = &Config::with_settings(global_args, request)?;

    config.self_update_if_needed().await;

    let progress = WorkProgress::new();

    let request = config.ruby_request();

    let (engine, version) = match request {
        RubyRequest::Dev => (RubyEngine::Ruby, "dev".to_string()),
        RubyRequest::Released(_) => {
            let matched = config.find_matching_remote_ruby().await?;
            (matched.engine.clone(), matched.number())
        }
    };

    warn_if_jvm_missing(&engine);

    let install_dir = match install_dir {
        Some(dir) => Utf8PathBuf::from(dir),
        None => match config.ruby_dirs.first() {
            Some(dir) => dir.clone(),
            None => panic!("No Ruby directories to install into"),
        },
    };

    if config.is_requested_ruby_installed_in_dir(&install_dir) && !force {
        println!("Version already installed. If you want to overwrite it, use '--force'.");

        return Ok(());
    }

    let archive_path = if let Some(path) = tarball_path {
        path
    } else {
        download_tarball(config, &engine, &version, &progress).await?
    };

    extract_ruby_archive(&archive_path, &install_dir, &engine, &version)?;

    let installed_version = if version == "dev" {
        "ruby-dev".cyan().to_string()
    } else if engine == RubyEngine::Ruby {
        format!("Ruby version {}", version.cyan())
    } else {
        format!("{engine} version {}", version.cyan())
    };

    println!("Installed {installed_version} to {}", install_dir.cyan());

    Ok(())
}

/// JRuby runs on the JVM, but rv doesn't install one. Without Java the install
/// still succeeds while the ruby stays unusable, so say so before we spend the
/// download rather than let `rv ruby find` come up empty later.
fn warn_if_jvm_missing(engine: &RubyEngine) {
    if engine != &RubyEngine::JRuby {
        return;
    }

    if std::env::var_os("JAVA_HOME").is_some() || which::which("java").is_ok() {
        return;
    }

    warn!("No JVM detected, but JRuby requires one. Install a JDK, or set JAVA_HOME.");
}

// downloads a remote ruby archive (tarball or zip)
async fn download_tarball(
    config: &Config,
    engine: &RubyEngine,
    version: &str,
    progress: &WorkProgress,
) -> Result<Utf8PathBuf> {
    let host = HostPlatform::current()?;
    let ext = archive_ext_for(engine, &host);
    let mut url = ruby_url(engine, version, &host);

    if version == "dev" && !host.is_windows() {
        url = find_latest_ruby_dev_url(&url).await?;
    }
    let archive_path = archive_cache_path(config, &url, ext);

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
        download_ruby_archive(config, &url, &archive_path, version, progress, ext).await?;
    }

    Ok(archive_path)
}

/// Does a usable archive already exist at this path?
fn valid_archive_exists(path: &Utf8Path) -> bool {
    fs_err::metadata(path).is_ok_and(|m| m.is_file() && m.len() > 0)
}

/// JRuby ships `.zip` on Windows and `.tar.gz` elsewhere, not RubyInstaller2's `.7z`.
fn archive_ext_for(engine: &RubyEngine, host: &HostPlatform) -> &'static str {
    match engine {
        RubyEngine::JRuby if host.is_windows() => "zip",
        RubyEngine::JRuby => "tar.gz",
        _ => host.archive_ext(),
    }
}

fn ruby_url(engine: &RubyEngine, version: &str, host: &HostPlatform) -> String {
    let download_base = std::env::var("RV_INSTALL_URL")
        .unwrap_or_else(|_| download_base_for(engine, version, host));
    let download_path = download_path_for(engine, version, host);

    format!("{download_base}/{download_path}")
}

fn download_base_for(engine: &RubyEngine, version: &str, host: &HostPlatform) -> String {
    match engine {
        RubyEngine::JRuby => "https://github.com/jruby/jruby/releases/download".to_owned(),
        _ if host.is_windows() => {
            "https://github.com/oneclick/rubyinstaller2/releases/download".to_owned()
        }
        _ if version == "dev" => {
            "https://github.com/spinel-coop/rv-ruby-dev/releases/latest/download".to_owned()
        }
        _ => "https://github.com/spinel-coop/rv-ruby/releases/latest/download".to_owned(),
    }
}

fn download_path_for(engine: &RubyEngine, version: &str, host: &HostPlatform) -> String {
    let arch = host.ruby_arch_str();
    let ext = archive_ext_for(engine, host);

    // JRuby archives are universal, so the path carries no arch.
    if *engine == RubyEngine::JRuby {
        return format!("{version}/jruby-bin-{version}.{ext}");
    }

    if host.is_windows() {
        if version == "dev" {
            // Dev builds use the rubyinstaller-head release (no revision number)
            format!("rubyinstaller-head/rubyinstaller-head-{arch}.{ext}")
        } else {
            format!("RubyInstaller-{version}-1/rubyinstaller-{version}-1-{arch}.{ext}")
        }
    } else {
        format!("ruby-{version}.{arch}.{ext}")
    }
}

async fn find_latest_ruby_dev_url(url: &str) -> Result<String> {
    let redirects = false;
    let response = fetch_url(url, redirects).await?;

    if response.status() == StatusCode::FOUND {
        Ok(response
            .headers()
            .get(reqwest::header::LOCATION)
            .expect("a redirect response should have a location header")
            .to_str()
            .expect("location header should be a valid UTF-8 string")
            .to_string())
    } else {
        Err(Error::GetLatestDevReleaseFailed)
    }
}

fn archive_cache_path(config: &Config, url: impl AsRef<str>, ext: &str) -> Utf8PathBuf {
    let cache_key = rv_cache::cache_digest(url.as_ref());
    config
        .cache
        .shard(rv_cache::CacheBucket::Ruby, "tarballs")
        .into_path_buf()
        .join(format!("{cache_key}.{ext}"))
}

fn temp_archive_path(config: &Config, url: impl AsRef<str>, ext: &str) -> Utf8PathBuf {
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
    version: &str,
    progress: &WorkProgress,
    ext: &str,
) -> Result<()> {
    debug!("Downloading archive from {url}");
    let redirects = true;
    let response = fetch_url(url, redirects).await?;

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

    let span = info_span!("Downloading Ruby", version);
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name} {msg}").unwrap());
    let _guard = span.enter();

    // Write the archive bytes to the filesystem.
    let temp_path = temp_archive_path(config, url, ext);
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

async fn fetch_url(url: &str, redirects: bool) -> Result<reqwest::Response> {
    // Build the request with optional GitHub authentication
    let client = if !redirects {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?
    } else {
        reqwest::Client::new()
    };

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

    Ok(request_builder.send().await?)
}

/// Must match what `Config::is_requested_ruby_installed_in_dir` looks for.
fn install_dir_name(engine: &RubyEngine, version: &str) -> String {
    format!("{engine}-{version}")
}

/// rv-ruby tarballs nest as `rv-ruby@{version}/{version}/…`; JRuby's have a single root.
fn tarball_strip_components(engine: &RubyEngine) -> usize {
    match engine {
        RubyEngine::JRuby => 1,
        _ => 2,
    }
}

fn extract_ruby_archive(
    archive_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    engine: &RubyEngine,
    version: &str,
) -> Result<()> {
    let host = HostPlatform::current()?;
    let span = info_span!("Installing Ruby", version);
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name}").unwrap());
    let _guard = span.enter();

    if !rubies_dir.exists() {
        fs_err::create_dir_all(rubies_dir)?;
    }

    let dir_name = install_dir_name(engine, version);

    // Determine archive type by extension
    let extension = archive_path.extension().unwrap_or("");
    match extension {
        // Both zip sources, RubyInstaller2 and JRuby on Windows, have a single root.
        "zip" => extract_zip(archive_path, rubies_dir, &dir_name, 1),
        "7z" => extract_7z(archive_path, rubies_dir, &dir_name, version, &host),
        _ => extract_tarball(
            archive_path,
            rubies_dir,
            &dir_name,
            tarball_strip_components(engine),
        ),
    }
}

fn extract_tarball(
    tarball_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    dir_name: &str,
    strip_components: usize,
) -> Result<()> {
    let tarball = fs_err::File::open(tarball_path)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    let dst_dir: PathBuf = rubies_dir.as_std_path().join(dir_name);

    let mut long_name: Option<PathBuf> = None;

    for e in archive.entries()? {
        let mut entry = e?;

        // Paths over 100 bytes live in a preceding `././@LongLink` record. The tar
        // crate only applies those under GNU or ustar magic, but JRuby's tarballs
        // write `ustar` magic with a zeroed version field, which is neither, so the
        // record surfaces here and the real entry arrives with its name truncated to
        // 100 bytes. Apply the name ourselves.
        match entry.header().entry_type() {
            tar::EntryType::GNULongName => {
                let mut name = String::new();
                entry.read_to_string(&mut name)?;
                long_name = Some(PathBuf::from(name.trim_end_matches('\0')));
                continue;
            }
            // Same, for long symlink targets. Unused here; skip so it isn't written out.
            tar::EntryType::GNULongLink => continue,
            _ => {}
        }

        let entry_path = match long_name.take() {
            Some(name) => Cow::Owned(name),
            None => entry.path()?,
        };

        let mut dst_file = dst_dir.to_path_buf();

        // Strip the archive's own root directories
        let mut path = entry_path.components();
        for _ in 0..strip_components {
            path.next();
        }

        // Copied from
        // https://github.com/composefs/tar-rs/blob/fc459c149f83bf4daceaa52e17d351989002e1a9/src/entry.rs#L404-L419,
        // xcept we raise an error if we find a path with ".." inside the archive, rather than
        // skipping extraction of that particular file.
        for part in path {
            match part {
                // Leading '/' characters, root paths, and '.'
                // components are just ignored and treated as "empty
                // components"
                Component::Prefix(..) | Component::RootDir | Component::CurDir => continue,

                // If any part of the filename is '..', then skip over
                // unpacking the file to prevent directory traversal
                // security issues.  See, e.g.: CVE-2001-1267,
                // CVE-2002-0399, CVE-2005-1918, CVE-2007-4131
                Component::ParentDir => {
                    return Err(Error::DirectoryTraversalError(
                        entry_path.display().to_string(),
                    ));
                }

                Component::Normal(part) => dst_file.push(part),
            }
        }

        // `Entry::unpack` won't create parents the way `Archive::unpack` does, and
        // JRuby's tarball lists files before their directory entries.
        if let Some(parent) = dst_file.parent() {
            fs_err::create_dir_all(parent)?;
        }

        crate::tar_utils::unpack_entry(&mut entry, &dst_file)?;
    }
    Ok(())
}

fn extract_zip(
    zip_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    dir_name: &str,
    strip_components: usize,
) -> Result<()> {
    let file = fs_err::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let dst_dir = rubies_dir.join(dir_name);
    fs_err::create_dir_all(&dst_dir)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let path = entry.name().replace('\\', "/"); // Normalize Windows path separators

        if path.contains("..") {
            return Err(Error::DirectoryTraversalError(path));
        }

        // Rebuild under `dir_name` so the archive's own root doesn't name the install.
        let relative: Vec<&str> = path
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .skip(strip_components)
            .collect();

        if relative.is_empty() {
            continue;
        }

        let dst = dst_dir.join(relative.join("/"));

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

fn entry_extract_fn(
    entry: &sevenz_rust2::ArchiveEntry,
    reader: &mut dyn std::io::Read,
    dest: &PathBuf,
) -> std::result::Result<bool, sevenz_rust2::Error> {
    sevenz_rust2::default_entry_extract_fn(entry, reader, dest)
}

fn extract_7z(
    archive_path: &Utf8Path,
    rubies_dir: &Utf8Path,
    dir_name: &str,
    version: &str,
    host: &HostPlatform,
) -> Result<()> {
    // Extract 7z archive to rubies_dir
    sevenz_rust2::decompress_file_with_extract_fn(
        archive_path.as_std_path(),
        rubies_dir.as_std_path(),
        entry_extract_fn,
    )?;

    // RubyInstaller2 extracts to: rubyinstaller-{request}-1-{arch}/
    // Dev builds extract to: rubyinstaller-head-{arch}/ (no revision number)
    // We need to rename it to: ruby-{request}/
    let arch = host.ruby_arch_str();
    let extracted_dir = if version == "dev" {
        rubies_dir.join(format!("rubyinstaller-head-{arch}"))
    } else {
        rubies_dir.join(format!("rubyinstaller-{}-1-{arch}", version))
    };
    let target_dir = rubies_dir.join(dir_name);

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

    #[test]
    fn test_ruby_url_unix() {
        let host = HostPlatform::from_target_triple("aarch64-apple-darwin").unwrap();
        let url = ruby_url(&RubyEngine::Ruby, "3.4.1", &host);

        assert_eq!(
            url,
            "https://github.com/spinel-coop/rv-ruby/releases/latest/download/ruby-3.4.1.arm64_sonoma.tar.gz"
        );
    }

    #[test]
    fn test_ruby_url_windows() {
        let host = HostPlatform::from_target_triple("x86_64-pc-windows-msvc").unwrap();
        let url = ruby_url(&RubyEngine::Ruby, "3.4.1", &host);

        assert_eq!(
            url,
            "https://github.com/oneclick/rubyinstaller2/releases/download/RubyInstaller-3.4.1-1/rubyinstaller-3.4.1-1-x64.7z"
        );
    }

    #[test]
    fn test_ruby_url_windows_arm64() {
        let host = HostPlatform::from_target_triple("aarch64-pc-windows-msvc").unwrap();
        let url = ruby_url(&RubyEngine::Ruby, "3.4.1", &host);

        assert_eq!(
            url,
            "https://github.com/oneclick/rubyinstaller2/releases/download/RubyInstaller-3.4.1-1/rubyinstaller-3.4.1-1-arm.7z"
        );
    }

    #[test]
    fn test_ruby_url_unix_dev() {
        let host = HostPlatform::from_target_triple("aarch64-apple-darwin").unwrap();
        let url = ruby_url(&RubyEngine::Ruby, "dev", &host);

        assert_eq!(
            url,
            "https://github.com/spinel-coop/rv-ruby-dev/releases/latest/download/ruby-dev.arm64_sonoma.tar.gz"
        );
    }

    #[test]
    fn test_ruby_url_windows_dev() {
        let host = HostPlatform::from_target_triple("x86_64-pc-windows-msvc").unwrap();
        let url = ruby_url(&RubyEngine::Ruby, "dev", &host);

        assert_eq!(
            url,
            "https://github.com/oneclick/rubyinstaller2/releases/download/rubyinstaller-head/rubyinstaller-head-x64.7z"
        );
    }

    #[test]
    fn test_jruby_url_is_universal_across_unix_platforms() {
        // One archive for every platform, so the URL must not vary with host arch.
        for triple in [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-musl",
        ] {
            let host = HostPlatform::from_target_triple(triple).unwrap();
            let url = ruby_url(&RubyEngine::JRuby, "10.1.1.0", &host);

            assert_eq!(
                url,
                "https://github.com/jruby/jruby/releases/download/10.1.1.0/jruby-bin-10.1.1.0.tar.gz",
                "wrong JRuby url for {triple}"
            );
        }
    }

    #[test]
    fn test_jruby_url_windows_uses_zip() {
        let host = HostPlatform::from_target_triple("x86_64-pc-windows-msvc").unwrap();
        let url = ruby_url(&RubyEngine::JRuby, "9.4.15.0", &host);

        assert_eq!(
            url,
            "https://github.com/jruby/jruby/releases/download/9.4.15.0/jruby-bin-9.4.15.0.zip"
        );
    }

    #[test]
    fn test_archive_ext_for() {
        let mac = HostPlatform::from_target_triple("aarch64-apple-darwin").unwrap();
        let win = HostPlatform::from_target_triple("x86_64-pc-windows-msvc").unwrap();

        assert_eq!(archive_ext_for(&RubyEngine::Ruby, &mac), "tar.gz");
        assert_eq!(archive_ext_for(&RubyEngine::Ruby, &win), "7z");
        assert_eq!(archive_ext_for(&RubyEngine::JRuby, &mac), "tar.gz");
        assert_eq!(archive_ext_for(&RubyEngine::JRuby, &win), "zip");
    }

    #[test]
    fn test_install_dir_name_matches_request_display() {
        use rv_ruby::request::RubyRequest;
        use std::str::FromStr;

        // The directory rv extracts into has to be the one it later looks for.
        for request in ["ruby-3.4.1", "jruby-10.1.1.0", "jruby-9.4.12.1"] {
            let parsed = RubyRequest::from_str(request).unwrap();
            let version = rv_ruby::version::RubyVersion::try_from(parsed).unwrap();

            assert_eq!(
                install_dir_name(&version.engine, &version.number()),
                request
            );
        }
    }

    #[test]
    fn test_tarball_strip_components() {
        assert_eq!(tarball_strip_components(&RubyEngine::Ruby), 2);
        assert_eq!(tarball_strip_components(&RubyEngine::JRuby), 1);
    }

    #[test]
    fn test_extract_zip_uses_the_given_dir_name_not_the_archive_root() {
        // The archive's own root must not decide the install dir name.
        let temp_dir = TempDir::new().unwrap();
        let rubies_dir = temp_dir.child("rubies");
        rubies_dir.create_dir_all().unwrap();

        let zip_path = temp_dir.child("jruby.zip");
        {
            let file = std::fs::File::create(zip_path.path()).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::SimpleFileOptions = Default::default();
            zip.add_directory::<_, ()>("some-unexpected-root/", options)
                .unwrap();
            zip.add_directory::<_, ()>("some-unexpected-root/bin/", options)
                .unwrap();
            zip.start_file("some-unexpected-root/bin/ruby.bat", options)
                .unwrap();
            zip.write_all(b"fake jruby launcher").unwrap();
            zip.finish().unwrap();
        }

        extract_zip(
            Utf8Path::from_path(zip_path.path()).unwrap(),
            Utf8Path::from_path(rubies_dir.path()).unwrap(),
            &install_dir_name(&RubyEngine::JRuby, "10.1.1.0"),
            1,
        )
        .unwrap();

        let launcher = rubies_dir
            .child("jruby-10.1.1.0")
            .child("bin")
            .child("ruby.bat");
        assert!(
            launcher.exists(),
            "zip should land under the requested dir name regardless of its own root"
        );
        assert_eq!(
            std::fs::read_to_string(launcher.path()).unwrap(),
            "fake jruby launcher"
        );
    }

    #[test]
    fn test_extract_zip_rejects_directory_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let rubies_dir = temp_dir.child("rubies");
        rubies_dir.create_dir_all().unwrap();

        let zip_path = temp_dir.child("evil.zip");
        {
            let file = std::fs::File::create(zip_path.path()).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options: zip::write::SimpleFileOptions = Default::default();
            zip.start_file("root/../../escaped.txt", options).unwrap();
            zip.write_all(b"nope").unwrap();
            zip.finish().unwrap();
        }

        let result = extract_zip(
            Utf8Path::from_path(zip_path.path()).unwrap(),
            Utf8Path::from_path(rubies_dir.path()).unwrap(),
            "ruby-3.4.1",
            1,
        );
        assert!(matches!(result, Err(Error::DirectoryTraversalError(_))));
    }

    #[test]
    fn test_extract_tarball_strips_single_root_for_jruby() {
        use flate2::{Compression, write::GzEncoder};

        let mut tar_data = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_data);

            for dir in ["jruby-10.1.1.0/", "jruby-10.1.1.0/bin/"] {
                let mut header = tar::Header::new_gnu();
                header.set_path(dir).unwrap();
                header.set_size(0);
                header.set_mode(0o755);
                header.set_entry_type(tar::EntryType::Directory);
                header.set_cksum();
                builder.append(&header, std::io::empty()).unwrap();
            }

            let contents = b"#!/bin/sh\n";
            let mut header = tar::Header::new_gnu();
            header.set_path("jruby-10.1.1.0/bin/ruby").unwrap();
            header.set_size(contents.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append(&header, &contents[..]).unwrap();
            builder.finish().unwrap();
        }
        let mut gz = Vec::new();
        {
            let mut encoder = GzEncoder::new(&mut gz, Compression::default());
            encoder.write_all(&tar_data).unwrap();
            encoder.finish().unwrap();
        }

        let temp_dir = TempDir::new().unwrap();
        let rubies_dir = temp_dir.child("rubies");
        rubies_dir.create_dir_all().unwrap();
        let archive = temp_dir.child("jruby-bin-10.1.1.0.tar.gz");
        archive.write_binary(&gz).unwrap();

        extract_tarball(
            Utf8Path::from_path(archive.path()).unwrap(),
            Utf8Path::from_path(rubies_dir.path()).unwrap(),
            &install_dir_name(&RubyEngine::JRuby, "10.1.1.0"),
            tarball_strip_components(&RubyEngine::JRuby),
        )
        .unwrap();

        let ruby_bin = rubies_dir
            .child("jruby-10.1.1.0")
            .child("bin")
            .child("ruby");
        assert!(
            ruby_bin.exists(),
            "jruby-10.1.1.0/bin/ruby should exist after extraction"
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
        extract_zip(zip_utf8_path, rubies_path, "ruby-3.4.1", 1).unwrap();

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

        let result = extract_ruby_archive(zip_utf8_path, rubies_path, &RubyEngine::Ruby, "3.4.1");
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
