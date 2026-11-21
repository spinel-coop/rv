use anstream::println;
use bytesize::ByteSize;
use camino::{Utf8Path, Utf8PathBuf};
use core::panic;
use futures_util::StreamExt;
use indicatif::ProgressStyle;
use owo_colors::OwoColorize;
use reqwest::StatusCode;
use sha2::{Digest as _, Sha256};
use std::borrow::Cow;
use std::io::Read as _;
use std::path::{Component, PathBuf};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info_span, warn};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use rv_platform::HostPlatform;
use rv_ruby::engine::RubyEngine;
use rv_ruby::request::RubyRequest;
use rv_ruby::{IndexEntry, RemoteRuby};

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
    #[error(
        "The archive downloaded from {url} is damaged: its SHA-256 checksum is {actual}, but it should be {expected}"
    )]
    ChecksumMismatch {
        url: String,
        expected: String,
        actual: String,
    },
    #[error("Paths including .. are not allowed inside archives, but found {0}")]
    DirectoryTraversalError(String),
    #[error(transparent)]
    UnsupportedPlatform(#[from] rv_platform::UnsupportedPlatformError),
}

type Result<T> = miette::Result<T, Error>;

/// What the index says about the archive to download.
struct Listing {
    url: String,
    build: String,
    sha256: String,
}

impl From<RemoteRuby> for Listing {
    fn from(ruby: RemoteRuby) -> Self {
        Self {
            url: ruby.url,
            build: ruby.build,
            sha256: ruby.sha256,
        }
    }
}

impl From<IndexEntry> for Listing {
    fn from(entry: IndexEntry) -> Self {
        Self {
            url: entry.url,
            build: entry.build,
            sha256: entry.sha256,
        }
    }
}

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

    let (engine, version, remote) = match request {
        RubyRequest::Dev => (RubyEngine::Ruby, "dev".to_string(), None),
        RubyRequest::Released(_) => {
            let (matched, remote) = config.find_matching_remote_ruby_with_details().await?;
            (matched.engine.clone(), matched.number(), remote)
        }
    };

    // An exact version wasn't looked up in the index, but a copy of the index that we
    // already have may list it. Otherwise the alias for the version finds its newest build.
    let listing = remote.map(Listing::from).or_else(|| {
        config
            .cached_index_entry(&engine, &version)
            .map(Listing::from)
    });

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
        download_tarball(config, &engine, &version, listing.as_ref(), &progress).await?
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
/// Downloads the archive for a ruby, unless the cache already has it, and returns its path.
///
/// `listing` is what the index says about this ruby. Without it, the dateless alias for the
/// version serves its newest build in a single request.
async fn download_tarball(
    config: &Config,
    engine: &RubyEngine,
    version: &str,
    listing: Option<&Listing>,
    progress: &WorkProgress,
) -> Result<Utf8PathBuf> {
    let host = HostPlatform::current()?;
    let ext = archive_ext_for(engine, &host);

    let (url, build) = match listing {
        Some(listing) => (listing.url.clone(), Some(listing.build.clone())),
        None => (alias_url(engine, version, &host), None),
    };

    let cache_dir = config
        .cache
        .shard(rv_cache::CacheBucket::Ruby, "tarballs")
        .into_path_buf();
    if !cache_dir.exists() {
        fs_err::create_dir_all(&cache_dir)?;
    }

    // An alias doesn't say which build it serves until we ask for it.
    if let Some(build) = &build {
        let archive_path = archive_cache_path(config, &url, build, ext);
        if valid_archive_exists(&archive_path) {
            println!(
                "Archive {} already exists, skipping download.",
                archive_path.cyan()
            );
            return Ok(archive_path);
        }
    }

    download_ruby_archive(config, &url, listing, version, progress, ext).await
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

/// The URL that serves the newest build of a ruby for `host`, whichever build that is.
fn alias_url(engine: &RubyEngine, version: &str, host: &HostPlatform) -> String {
    let base = std::env::var("RV_INSTALL_URL").unwrap_or_else(|_| "https://rv.dev/ruby".to_owned());
    let platform = host.index_str();

    // CRuby sits at the root, and every other engine in a directory of its own.
    match engine {
        RubyEngine::Ruby => format!("{base}/{version}/{platform}"),
        _ => format!("{base}/{engine}/{version}/{platform}"),
    }
}

/// Where an archive is kept in the cache. The build is part of the key because an alias
/// URL stays the same when a ruby is rebuilt.
fn archive_cache_path(config: &Config, url: &str, build: &str, ext: &str) -> Utf8PathBuf {
    let cache_key = rv_cache::cache_digest(format!("{url}{build}"));
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
/// Then once the download finishes, the file will be renamed to `path`,
/// unless it doesn't have the `sha256` checksum it is expected to.
async fn write_to_filesystem(
    response: reqwest::Response,
    temp_path: &Utf8Path,
    path: &Utf8Path,
    sha256: Option<&str>,
    total_size: u64,
    progress: &WorkProgress,
    span: &tracing::Span,
) -> Result<()> {
    let url = response.url().to_string();
    let mut file = tokio::fs::File::create(&temp_path).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let chunk_len = chunk.len() as u64;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);

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

    // This catches a download that was cut short or corrupted on the way. It is no defence
    // against a server that lies, which could lie about the checksum as well.
    match sha256 {
        Some(expected) => {
            let actual = hex::encode(hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(Error::ChecksumMismatch {
                    url,
                    expected: expected.to_owned(),
                    actual,
                });
            }
        }
        None => debug!("No checksum is known for {url}, so the download was not verified"),
    }

    tokio::fs::rename(temp_path, path).await?;
    Ok(())
}

/// Downloads `url` into the archive cache and returns where it was put.
///
/// The build and checksum are known when the URL came from the index, as `listing`. An alias
/// names them in the `x-rv-build` and `x-rv-sha256` headers instead, and the cache may turn
/// out to have that build already.
async fn download_ruby_archive(
    config: &Config,
    url: &str,
    listing: Option<&Listing>,
    version: &str,
    progress: &WorkProgress,
    ext: &str,
) -> Result<Utf8PathBuf> {
    debug!("Downloading archive from {url}");
    let response = reqwest::Client::new().get(url).send().await?;

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

    let (build, sha256) = match listing {
        Some(listing) => (listing.build.clone(), Some(listing.sha256.clone())),
        None => (
            header(&response, "x-rv-build").unwrap_or_default(),
            header(&response, "x-rv-sha256"),
        ),
    };
    let archive_path = archive_cache_path(config, url, &build, ext);
    if valid_archive_exists(&archive_path) {
        println!(
            "Archive {} already exists, skipping download.",
            archive_path.cyan()
        );
        return Ok(archive_path);
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
        &archive_path,
        sha256.as_deref(),
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

    Ok(archive_path)
}

fn header(response: &reqwest::Response, name: &str) -> Option<String> {
    let value = response.headers().get(name)?.to_str().ok()?;
    Some(value.to_owned())
}

/// Names the directory an install lands in, from the resolved version.
///
/// `Config::is_requested_ruby_installed_in_dir` builds its path from the *request*
/// instead, so the two only line up when the request names a full version: `rv ruby
/// install jruby-9.4` installs into `jruby-9.4.15.0` while the check looks for
/// `jruby-9.4`. That mismatch is shared with CRuby, and its only effect is that the
/// already-installed check in `install` misses and we re-extract over the same
/// directory — never an install to the wrong place.
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
        // except we raise an error if we find a path with ".." inside the archive, rather than
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

        // Nothing left after stripping the archive's root, so this entry *is* the
        // install directory. Writing it would clobber the directory itself and make
        // every later entry fail with a confusing IO error.
        if dst_file == dst_dir {
            continue;
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
        let path = entry.name()?.replace('\\', "/"); // Normalize Windows path separators

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
    fn test_alias_url_unix() {
        let host = HostPlatform::from_target_triple("aarch64-apple-darwin").unwrap();
        let url = alias_url(&RubyEngine::Ruby, "3.4.1", &host);

        assert_eq!(url, "https://rv.dev/ruby/3.4.1/macos-aarch64");
    }

    #[test]
    fn test_alias_url_windows() {
        let host = HostPlatform::from_target_triple("x86_64-pc-windows-msvc").unwrap();
        let url = alias_url(&RubyEngine::Ruby, "3.4.1", &host);

        assert_eq!(url, "https://rv.dev/ruby/3.4.1/windows-x86_64");
    }

    #[test]
    fn test_alias_url_windows_arm64() {
        let host = HostPlatform::from_target_triple("aarch64-pc-windows-msvc").unwrap();
        let url = alias_url(&RubyEngine::Ruby, "3.4.1", &host);

        assert_eq!(url, "https://rv.dev/ruby/3.4.1/windows-aarch64");
    }

    #[test]
    fn test_alias_url_prerelease() {
        let host = HostPlatform::from_target_triple("aarch64-apple-darwin").unwrap();
        let url = alias_url(&RubyEngine::Ruby, "4.1.0-preview1", &host);

        assert_eq!(url, "https://rv.dev/ruby/4.1.0-preview1/macos-aarch64");
    }

    #[test]
    fn test_alias_url_dev() {
        let host = HostPlatform::from_target_triple("x86_64-unknown-linux-musl").unwrap();
        let url = alias_url(&RubyEngine::Ruby, "dev", &host);

        assert_eq!(url, "https://rv.dev/ruby/dev/linux-musl-x86_64");
    }

    #[test]
    fn test_alias_url_puts_other_engines_in_their_own_directory() {
        let host = HostPlatform::from_target_triple("x86_64-apple-darwin").unwrap();
        let url = alias_url(&RubyEngine::JRuby, "10.1.1.0", &host);

        assert_eq!(url, "https://rv.dev/ruby/jruby/10.1.1.0/macos-x86_64");
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
