use anstream::println;
use bytes::Bytes;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use indicatif::ProgressStyle;
use owo_colors::OwoColorize;
use rv_client::http_client::rv_http_client;
use rv_lockfile::datatypes::ChecksumAlgorithm;
use sha2::Digest;
use tracing::{debug, info_span, warn};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::commands::clean_install::{find_lockfile_path, url_for_spec};
use crate::{GlobalArgs, config::Config};

#[derive(Args, Debug)]
pub struct VendorArgs {
    /// Path to the Gemfile (or directory containing one)
    #[arg(long, short)]
    pub gemfile: Option<Utf8PathBuf>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] crate::config::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Parse(#[from] rv_lockfile::ParseErrors),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Ci(#[from] crate::commands::clean_install::Error),
    #[error("Could not determine where to vendor gems")]
    InvalidVendorPath,
    #[error("`rv vendor` requires network access. Re-run without --offline.")]
    OfflineNotSupported,
    #[error(
        "File {filename} did not match the {algo} checksum locked for gem {gem_name} in Gemfile.lock"
    )]
    LockfileChecksumFail {
        filename: String,
        gem_name: String,
        algo: &'static str,
    },
}

type Result<T> = miette::Result<T, Error>;

pub(crate) async fn vendor(global_args: &GlobalArgs, args: VendorArgs) -> Result<()> {
    let config = Config::with_settings(global_args, None)?;

    if config.offline {
        return Err(Error::OfflineNotSupported);
    }

    config.self_update_if_needed().await;

    let lockfile_path = find_lockfile_path(&args.gemfile)?;
    let raw = tokio::fs::read_to_string(&lockfile_path).await?;
    let normalized = rv_lockfile::normalize_line_endings(&raw);
    let lockfile = rv_lockfile::parse(&normalized)?;

    let vendor_dir = config
        .bundler_settings
        .cache_path()
        .ok_or(Error::InvalidVendorPath)?;
    fs_err::create_dir_all(&vendor_dir)?;

    let client = rv_http_client("vendor")?;

    let span = info_span!("Vendoring gems");
    span.pb_set_style(
        &ProgressStyle::with_template("{spinner:.green} {span_name} {pos}/{len} - {msg}").unwrap(),
    );
    span.pb_set_length(lockfile.gem_spec_count() as u64);
    let _guard = span.enter();

    let mut downloaded = 0u64;
    let mut skipped = 0u64;

    for gem_source in &lockfile.gem {
        let Some(remote) = gem_source.remote else {
            debug!("Skipping gem source with no remote");
            continue;
        };

        for spec in &gem_source.specs {
            let filename = spec.release_tuple.package_name();
            let target = vendor_dir.join(&filename);

            if target.exists() {
                debug!("{} already vendored, skipping", filename);
                skipped += 1;
                span.pb_inc(1);
                continue;
            }

            let bytes = fetch_gem(&config, &client, remote, spec).await?;
            verify_checksum(&bytes, spec, &lockfile, &filename)?;
            write_atomically(&target, &bytes).await?;

            downloaded += 1;
            span.pb_inc(1);
            span.pb_set_message(&filename);
        }
    }

    drop(_guard);

    if !lockfile.git.is_empty() || !lockfile.path.is_empty() {
        warn!("`rv vendor` does not yet handle git or path sources; those gems were skipped.");
    }

    println!(
        "Vendored {} gem(s) to {} ({} already present)",
        downloaded.to_string().cyan(),
        vendor_dir.cyan(),
        skipped,
    );

    Ok(())
}

/// Fetch a gem's bytes, preferring the user-level rv cache when present so
/// repeated `rv vendor` runs don't re-download the same archive.
async fn fetch_gem(
    config: &Config,
    client: &reqwest::Client,
    remote: &str,
    spec: &rv_lockfile::datatypes::Spec,
) -> Result<Bytes> {
    let mut url = url_for_spec(remote, spec)?;
    let user_cache_path = config
        .cache
        .shard(rv_cache::CacheBucket::Gem, "gems")
        .into_path_buf()
        .join(format!("{}.gem", rv_cache::cache_digest(url.as_str())));

    if user_cache_path.exists() {
        debug!("Reusing gem from rv cache: {}", url);
        return Ok(Bytes::from(tokio::fs::read(&user_cache_path).await?));
    }

    if let Some(host) = url.host_str()
        && let Some(token) = config.bundler_settings.token_for(host)
    {
        let _ = url.set_username(&token);
    }

    debug!("Downloading {}", url);
    let bytes = client
        .get(url.clone())
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    Ok(bytes)
}

fn verify_checksum(
    bytes: &Bytes,
    spec: &rv_lockfile::datatypes::Spec,
    lockfile: &rv_lockfile::datatypes::GemfileDotLock<'_>,
    filename: &str,
) -> Result<()> {
    let Some(checksums) = &lockfile.checksums else {
        return Ok(());
    };
    let Some(entry) = checksums
        .iter()
        .find(|c| c.release_tuple == spec.release_tuple)
    else {
        return Ok(());
    };

    match entry.algorithm {
        ChecksumAlgorithm::None => Ok(()),
        ChecksumAlgorithm::Unknown(other) => {
            warn!("Unknown checksum algorithm {} for gem {}", other, filename);
            Ok(())
        }
        ChecksumAlgorithm::SHA256 => {
            let actual = sha2::Sha256::digest(bytes);
            if actual[..] != entry.value {
                return Err(Error::LockfileChecksumFail {
                    filename: filename.to_owned(),
                    gem_name: spec.release_tuple.full_name(),
                    algo: "sha256",
                });
            }
            Ok(())
        }
    }
}

/// Write to a sibling `.tmp` file then rename, so an interrupted vendor run
/// never leaves a half-written `.gem` for the next `rv ci` to find.
async fn write_atomically(target: &Utf8Path, bytes: &Bytes) -> Result<()> {
    let tmp = target.with_extension("gem.tmp");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, target).await?;
    Ok(())
}
