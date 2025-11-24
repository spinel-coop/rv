use bytes::Bytes;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use owo_colors::OwoColorize;
use reqwest::Client;
use rv_gem_types::Specification as GemSpecification;
use rv_lockfile::datatypes::ChecksumAlgorithm;
use rv_lockfile::datatypes::GemSection;
use rv_lockfile::datatypes::GemVersion;
use rv_lockfile::datatypes::GemfileDotLock;
use rv_lockfile::datatypes::Spec;
use sha2::Digest;
use sha2::Sha256;
use sha2::Sha512;
use tracing::debug;
use tracing::info;
use url::Url;

use crate::config::Config;
use std::collections::HashMap;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::path::PathBuf;

const FS_CONCURRENCY_LIMIT: usize = 10;

#[derive(clap_derive::Args)]
pub struct CiArgs {
    /// Maximum number of downloads that can be in flight at once.
    #[arg(short, long, default_value = "10")]
    pub max_concurrent_requests: usize,

    /// Validate the checksums from the gem server and gem itself.
    #[arg(long, default_value = "true")]
    pub validate_checksums: bool,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] rv_lockfile::ParseErrors),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("Invalid remote URL")]
    BadRemote {
        remote: String,
        err: url::ParseError,
    },
    #[error(transparent)]
    UrlError(#[from] url::ParseError),
    #[error("Could not read install directory from Bundler")]
    BadBundlePath,
    #[error("Failed to unpack tarball path {0}")]
    InvalidTarballPath(PathBuf),
    #[error("Checksum file was not valid YAML")]
    InvalidChecksum,
    #[error("Gem archive did not include metadata.gz")]
    NoMetadata,
    #[error("Gem archive did not include data.tar.gz")]
    NoDataTar,
    #[error("File {filename} did not match {algo} checksum in gem {gem_name}")]
    ChecksumFail {
        filename: String,
        gem_name: String,
        algo: &'static str,
    },
    #[error("Invalid gem archive: {0}")]
    InvalidGemArchive(String),
}

type Result<T> = std::result::Result<T, Error>;

pub async fn ci(config: &Config, args: CiArgs) -> Result<()> {
    let lockfile_path;
    if let Some(path) = &config.gemfile {
        lockfile_path = format!("{}.lock", path.clone()).into();
    } else {
        lockfile_path = "Gemfile.lock".into();
    }
    ci_inner(lockfile_path, &config.cache, &args).await
}

async fn ci_inner(
    lockfile_path: Utf8PathBuf,
    cache: &rv_cache::Cache,
    args: &CiArgs,
) -> Result<()> {
    let lockfile_contents = tokio::fs::read_to_string(lockfile_path).await?;
    let lockfile = rv_lockfile::parse(&lockfile_contents)?;
    let gems = download_gems(lockfile, cache, args).await?;
    let start_install = std::time::Instant::now();
    install_gems(gems, args).await?;
    let install_time = start_install.elapsed();
    eprintln!("ADAM: Install took {}s", install_time.as_secs_f64());
    Ok(())
}

fn find_bundle_path() -> Result<Utf8PathBuf> {
    let bundle_path = std::process::Command::new("ruby")
        .args(["-rbundler", "-e", "'puts Bundler.bundle_path'"])
        .spawn()?
        .wait_with_output()
        .map(|out| out.stdout)?;
    String::from_utf8(bundle_path)
        .map_err(|_| Error::BadBundlePath)
        .map(Utf8PathBuf::from)
}

async fn install_gems<'i>(downloaded: Vec<Downloaded<'i>>, args: &CiArgs) -> Result<()> {
    // 1. Get the path where we want to put the gems from Bundler
    //    ruby -rbundler -e 'puts Bundler.bundle_path'
    let bundle_path = find_bundle_path()?;
    // 2. Unpack all the tarballs
    let binstub_dir = bundle_path.join("bin");
    use futures_util::stream::TryStreamExt;
    let dep_gemspecs: Result<Vec<_>> = futures_util::stream::iter(downloaded)
        .map(|download| async {
            let dep_gemspec = download.unpack_tarball(bundle_path.clone(), args).await?;
            Ok(dep_gemspec)
        })
        .buffered(FS_CONCURRENCY_LIMIT)
        .and_then(|dep_gemspec| async {
            if let Some(dep_gemspec) = dep_gemspec {
                install_binstub(&dep_gemspec.name, &dep_gemspec.executables, &binstub_dir).await
            }
            Ok(())
        })
        .try_collect()
        .await;

    // 3. Generate binstubs into DIR/bin/
    // futures_util::stream::iter(dep_gemspecs)
    //     .for_each_concurrent(FS_CONCURRENCY_LIMIT, |(gem_version, dep_gemspec)| async {
    //         install_binstub(x, binstub_dir)
    //     })
    //     .await;

    // 4. Handle compiling native extensions for gems with native extensions
    // 5. Copy the .gem files and the .gemspec files into cache and specificatiosn?
    Ok(())
}

async fn install_binstub(dep_name: &str, executables: &[String], binstub_dir: &Utf8Path) {
    for exe_name in executables {
        if let Err(e) = write_binstub(&dep_name, &exe_name, &binstub_dir).await {
            tracing::warn!("Could not write binstub for {dep_name}/{exe_name}: {e}",);
        }
    }
}

fn rv_http_client() -> Result<Client> {
    use reqwest::header;
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "X-RV-PLATFORM",
        header::HeaderValue::from_static(current_platform::CURRENT_PLATFORM),
    );
    headers.insert("X-RV-COMMAND", header::HeaderValue::from_static("ci"));

    let client = reqwest::Client::builder()
        .user_agent(format!("rv-{}", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        .build()?;

    Ok(client)
}

enum KnownChecksumAlgos {
    Sha256,
}

struct HowToChecksum {
    algorithm: KnownChecksumAlgos,
    value: Vec<u8>,
}

/// Downloads all gems from a Gemfile.lock
async fn download_gems<'i>(
    lockfile: GemfileDotLock<'i>,
    cache: &rv_cache::Cache,
    args: &CiArgs,
) -> Result<Vec<Downloaded<'i>>> {
    let all_sources = futures_util::stream::iter(lockfile.gem);
    let checksums = if args.validate_checksums
        && let Some(checks) = lockfile.checksums
    {
        let mut hm = HashMap::new();
        for checksum in checks {
            hm.insert(
                checksum.gem_version,
                HowToChecksum {
                    algorithm: match checksum.algorithm {
                        ChecksumAlgorithm::None => continue,
                        ChecksumAlgorithm::Unknown(other) => {
                            eprintln!("Unknown checksum algorithm {}", other.yellow());
                            continue;
                        }
                        ChecksumAlgorithm::SHA256 => KnownChecksumAlgos::Sha256,
                    },
                    value: checksum.value,
                },
            );
        }
        hm
    } else {
        HashMap::default()
    };
    let downloaded: Vec<_> = all_sources
        .map(|gem_source| {
            download_gem_source(gem_source, &checksums, cache, args.max_concurrent_requests)
        })
        .buffered(args.max_concurrent_requests)
        .try_collect::<Vec<_>>()
        .await?
        .into_iter()
        .flatten()
        .collect();
    Ok(downloaded)
}

struct Downloaded<'i> {
    contents: Bytes,
    spec: Spec<'i>,
}

/// Checksums found in the gem under checksums.yaml
/// Note we do NOT check SHA1 as it is insecure.
#[derive(Debug, Default)]
struct ArchiveChecksums {
    sha256: Option<ChecksumFiles>,
    sha512: Option<ChecksumFiles>,
}

/// Checksums found in the gem under checksums.yaml
#[derive(Debug)]
struct ChecksumFiles {
    /// Expected checksum, given by server.
    metadata_gz: Vec<u8>,
    /// Expected checksum, given by server.
    data_tar_gz: Vec<u8>,
}

fn hex_key(yaml: &saphyr::Yaml<'_>) -> Option<Vec<u8>> {
    hex::decode(yaml.as_str()?).ok()
}

impl ArchiveChecksums {
    fn new(file: &str) -> Option<Self> {
        use saphyr::{LoadableYamlNode, Yaml};
        let contents_yaml = Yaml::load_from_str(file).ok()?;
        if contents_yaml.is_empty() {
            return None;
        }
        let root = &contents_yaml[0];
        let mut out = ArchiveChecksums::default();

        if let Some(checksums) = root.as_mapping_get("SHA256") {
            out.sha256 = Some(ChecksumFiles {
                metadata_gz: checksums.as_mapping_get("metadata.gz").and_then(hex_key)?,
                data_tar_gz: checksums.as_mapping_get("data.tar.gz").and_then(hex_key)?,
            });
        }
        if let Some(checksums) = root.as_mapping_get("SHA512") {
            out.sha512 = Some(ChecksumFiles {
                metadata_gz: checksums.as_mapping_get("metadata.gz").and_then(hex_key)?,
                data_tar_gz: checksums.as_mapping_get("data.tar.gz").and_then(hex_key)?,
            });
        }
        Some(out)
    }

    fn validate_data_tar(&self, gem_name: String, hashed: &Hashed) -> Result<()> {
        if self.sha256.is_none() && self.sha512.is_none() {
            eprintln!("Checksum file was empty");
        }
        if let Some(sha256) = &self.sha256
            && hashed.digest_256 != sha256.data_tar_gz
        {
            return Err(Error::ChecksumFail {
                filename: "data.tar.gz".to_owned(),
                gem_name,
                algo: "sha256",
            });
        }
        if let Some(sha512) = &self.sha512
            && hashed.digest_512 != sha512.data_tar_gz
        {
            return Err(Error::ChecksumFail {
                filename: "data.tar.gz".to_owned(),
                gem_name,
                algo: "sha512",
            });
        }
        Ok(())
    }

    fn validate_metadata(&self, gem_name: String, hashed: Hashed) -> Result<()> {
        if self.sha256.is_none() && self.sha512.is_none() {
            eprintln!("Checksum file was empty");
        }
        if let Some(sha256) = &self.sha256 {
            let expected = &sha256.metadata_gz;
            if hashed.digest_256 != expected {
                return Err(Error::ChecksumFail {
                    filename: "metadata.gz".to_owned(),
                    gem_name,
                    algo: "sha256",
                });
            }
        }
        if let Some(sha512) = &self.sha512
            && hashed.digest_512 != sha512.metadata_gz
        {
            return Err(Error::ChecksumFail {
                filename: "metadata.gz".to_owned(),
                gem_name,
                algo: "sha512",
            });
        }
        Ok(())
    }
}

impl<'i> Downloaded<'i> {
    async fn unpack_tarball(
        self,
        bundle_path: Utf8PathBuf,
        args: &CiArgs,
    ) -> Result<Option<GemSpecification>> {
        // Unpack the tarball into DIR/gems/
        // It should contain a metadata zip, and a data zip
        // (and optionally, a checksum zip).
        let GemVersion { name, version } = self.spec.gem_version;
        let nameversion = format!("{name}-{version}");
        debug!("Unpacking {nameversion}");

        // Then unpack the tarball into it.
        let contents = Cursor::new(self.contents);
        let mut archive = tar::Archive::new(contents.clone());

        let checksums = if args.validate_checksums {
            let mut checksums: Option<ArchiveChecksums> = None;
            for e in archive.entries()? {
                let entry = e?;
                let entry_path = entry.path()?;

                if entry_path.display().to_string().as_str() == "checksums.yaml.gz" {
                    let mut contents = GzDecoder::new(entry);
                    let mut str_contents = String::new();
                    let _ = contents.read_to_string(&mut str_contents)?;
                    let cs = ArchiveChecksums::new(&str_contents).ok_or(Error::InvalidChecksum)?;

                    if checksums.replace(cs).is_some() {
                        return Err(Error::InvalidGemArchive(
                            "two checksums.yaml.gz files found in the gem archive".to_owned(),
                        ));
                    }
                }
            }
            if checksums.is_none() {
                eprintln!(
                    "Warning: No checksums found for gem {}",
                    nameversion.yellow()
                );
            }
            checksums
        } else {
            None
        };

        let mut found_metadata = false;
        let mut unpacked_data_tar = None;
        let mut archive = tar::Archive::new(contents);
        for e in archive.entries()? {
            let entry = e?;
            let entry_path = entry.path()?;
            match entry_path.display().to_string().as_str() {
                "metadata.gz" => {
                    if found_metadata {
                        return Err(Error::InvalidGemArchive("two metadata.gz found".to_owned()));
                    }
                    found_metadata = true;
                    let data = HashReader::new(entry);
                    let UnpackedMetdata { hashed, extensions } =
                        unpack_metadata(&bundle_path, &nameversion, data).await?;
                    if !extensions.is_empty() {
                        tracing::debug!("Found native extensions, compiling them");
                        compile_native_extensions(extensions)?;
                    }
                    if args.validate_checksums
                        && let Some(ref checksums) = checksums
                    {
                        checksums.validate_metadata(nameversion.clone(), hashed)?
                    }
                }
                "data.tar.gz" => {
                    if unpacked_data_tar.is_some() {
                        return Err(Error::InvalidGemArchive("two data.tar.gz found".to_owned()));
                    }
                    let data = HashReader::new(entry);
                    let unpacked = unpack_data_tar(&bundle_path, name, &nameversion, data).await?;
                    if args.validate_checksums
                        && let Some(ref checksums) = checksums
                    {
                        checksums.validate_data_tar(nameversion.clone(), &unpacked.hashed)?
                    }
                    unpacked_data_tar = Some(unpacked);
                }
                "checksums.yaml.gz" => {
                    // Already handled
                }
                "data.tar.gz.sig" | "metadata.gz.sig" | "checksums.yaml.gz.sig" => {
                    // In the future, maybe we should add a flag which checks these?
                    // But I don't think anyone uses these in practice?
                    // Consider adding optional validation in the future.
                }
                other => {
                    info!("Unknown dir {other} in gem")
                }
            }
        }

        if !found_metadata {
            return Err(Error::NoMetadata);
        }
        let Some(unpacked_data) = unpacked_data_tar else {
            return Err(Error::NoDataTar);
        };

        Ok(unpacked_data.gemspec)
    }
}

async fn write_binstub(gem_name: &str, exe_name: &str, binstub_dir: &Utf8Path) -> io::Result<()> {
    let binstub_path = binstub_dir.join(exe_name);
    let binstub_contents =
        format!("require 'rubygems';\nGem.activate_and_load_bin_path('{gem_name}', '{exe_name}')",);
    tokio::fs::write(binstub_path, binstub_contents).await
}

fn compile_native_extensions(extensions: Vec<String>) -> Result<()> {
    for extension in extensions {
        let _compile_res = std::process::Command::new("ruby")
            .arg(extension)
            .spawn()?
            .wait_with_output();
        // TODO: check if this succeeded or not,
        // It should produce .so or .bundle files, which can be
        // copied to a specific place, so they can be loaded later.
    }
    Ok(())
}

struct HashReader<R> {
    reader: R,
    h256: Sha256,
    h512: Sha512,
}

struct Hashed {
    digest_256: Bytes,
    digest_512: Bytes,
}

impl<R> std::io::Read for HashReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.reader.read(buf)?;
        if n > 0 {
            self.h256.update(&buf[..n]);
            self.h512.update(&buf[..n]);
        }
        Ok(n)
    }
}

impl<R> HashReader<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            h256: Default::default(),
            h512: Default::default(),
        }
    }

    /// Get the final hash.
    fn finalize(self) -> Hashed {
        Hashed {
            digest_256: self.h256.finalize().to_vec().into(),
            digest_512: self.h512.finalize().to_vec().into(),
        }
    }
}

/// Result of unpacking a gem's `data.tar.gz` archive.
struct UnpackedData {
    hashed: Hashed,
    gemspec: Option<GemSpecification>,
}

/// Given the data.tar.gz from a gem, unpack its contents to the filesystem under
/// BUNDLEPATH/gems/name-version/ENTRY
/// Returns the checksum.
async fn unpack_data_tar<R>(
    bundle_path: &Utf8Path,
    gem_name: &str,
    nameversion: &str,
    data_tar_gz: HashReader<R>,
) -> Result<UnpackedData>
where
    R: std::io::Read,
{
    // First, create the data's destination.
    let data_dir: PathBuf = bundle_path.join("gems").join(nameversion).into();
    tokio::fs::create_dir_all(&data_dir).await?;
    let mut gem_data_archive = tar::Archive::new(GzDecoder::new(data_tar_gz));
    let mut gemspec = None;
    for e in gem_data_archive.entries()? {
        let mut entry = e?;
        let entry_path = entry.path()?;
        let entry_path_str = entry_path.to_string_lossy().to_lowercase();
        let dst = data_dir.join(entry_path);

        // Not sure if this is strictly necessary, or if we can know the
        // intermediate directories ahead of time.
        if let Some(dst_parent) = dst.parent() {
            tokio::fs::create_dir_all(dst_parent).await?;
        }
        entry.unpack(&dst)?;
        if entry_path_str == format!("{gem_name}.gemspec") {
            let gemspec_contents = tokio::fs::read_to_string(dst).await.map_err(Error::Io)?;
            match rv_gem_specification_yaml::parse(&gemspec_contents) {
                Ok(parsed) => {
                    gemspec = Some(parsed);
                }
                Err(e) => {
                    tracing::warn!("Invalid gemspec found in {nameversion}: {e}");
                }
            }
        };
    }
    // Get the HashReader back.
    let h = gem_data_archive.into_inner().into_inner();
    let hashed = h.finalize();
    Ok(UnpackedData { hashed, gemspec })
}

struct UnpackedMetdata {
    hashed: Hashed,
    extensions: Vec<String>,
}

/// Given the metadata.gz from a gem, write it to the filesystem under
/// BUNDLEPATH/specifications/name-version.gemspec
async fn unpack_metadata<R>(
    bundle_path: &Utf8Path,
    nameversion: &str,
    metadata_gz: HashReader<R>,
) -> Result<UnpackedMetdata>
where
    R: Read,
{
    // First, create the metadata's destination.
    let metadata_dir = bundle_path.join("specifications/");
    tokio::fs::create_dir_all(&metadata_dir).await?;
    let filename = format!("{nameversion}.gemspec");
    let dst_path = metadata_dir.join(filename);
    let mut dst = tokio::fs::File::create(&dst_path).await?;

    // Then write the (unzipped) source into the destination.
    let mut contents = String::new();
    let mut unzipper = GzDecoder::new(metadata_gz);
    unzipper.read_to_string(&mut contents)?;
    let parsed = match rv_gem_specification_yaml::parse(&contents) {
        Ok(parsed) => Some(parsed),
        Err(e) => {
            eprintln!("Warning: specification of {nameversion} was invalid: {e}");
            None
        }
    };
    tokio::io::copy(&mut Cursor::new(contents), &mut dst).await?;

    let extensions = parsed.map(|p| p.extensions).unwrap_or_default();

    let h = unzipper.into_inner();
    Ok(UnpackedMetdata {
        hashed: h.finalize(),
        extensions,
    })
}

fn url_for_spec(remote: &str, spec: &Spec<'_>) -> Result<Url> {
    let gem_name = spec.gem_version.name;
    let gem_version = spec.gem_version.version;
    let path = format!("gems/{gem_name}-{gem_version}.gem");
    let url = url::Url::parse(remote)
        .map_err(|err| Error::BadRemote {
            remote: remote.to_owned(),
            err,
        })?
        .join(&path)?;
    Ok(url)
}

/// Downloads all gems from a particular gem source,
/// e.g. from gems.coop or rubygems or something.
async fn download_gem_source<'i>(
    gem_source: GemSection<'i>,
    checksums: &HashMap<GemVersion<'i>, HowToChecksum>,
    cache: &rv_cache::Cache,
    max_concurrent_requests: usize,
) -> Result<Vec<Downloaded<'i>>> {
    // TODO: If the gem server needs user credentials, accept them and add them to this client.
    let client = rv_http_client()?;

    // Download them all, concurrently.
    let spec_stream = futures_util::stream::iter(gem_source.specs);
    let downloaded_gems: Vec<_> = spec_stream
        .map(|spec| download_gem(gem_source.remote, spec, &client, cache, checksums))
        .buffered(max_concurrent_requests)
        .try_collect()
        .await?;
    Ok(downloaded_gems)
}

/// Download a single gem, from the given URL, using the given client.
async fn download_gem<'i>(
    remote: &str,
    spec: Spec<'i>,
    client: &Client,
    cache: &rv_cache::Cache,
    checksums: &HashMap<GemVersion<'i>, HowToChecksum>,
) -> Result<Downloaded<'i>> {
    let url = url_for_spec(remote, &spec)?;
    let cache_key = rv_cache::cache_digest(url.as_ref());
    let cache_path = cache
        .shard(rv_cache::CacheBucket::Gem, "gems")
        .into_path_buf()
        .join(format!("{cache_key}.gem"));

    let contents;
    if cache_path.exists() {
        let data = tokio::fs::read(&cache_path).await?;
        contents = Bytes::from(data);
        debug!("Reusing gem from {url} in cache");
    } else {
        contents = client.get(url.clone()).send().await?.bytes().await?;
        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&cache_path, &contents).await?;
        debug!("Downloaded gem from {url}");
    }

    // Validate the checksums.
    if let Some(checksum) = checksums.get(&spec.gem_version) {
        match checksum.algorithm {
            KnownChecksumAlgos::Sha256 => {
                let actual = sha2::Sha256::digest(&contents);
                if actual[..] != checksum.value {
                    return Err(Error::ChecksumFail {
                        filename: url.to_string(),
                        gem_name: format!("{}-{}", spec.gem_version.name, spec.gem_version.version),
                        algo: "sha256",
                    });
                }
            }
        }
    }
    Ok(Downloaded { contents, spec })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ci_inner() -> Result<()> {
        let file = "../rv-lockfile/tests/inputs/Gemfile.lock.empty".into();
        let cache = rv_cache::Cache::temp().unwrap();
        ci_inner(
            file,
            &cache,
            &CiArgs {
                max_concurrent_requests: 10,
                validate_checksums: true,
            },
        )
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_ci_inner_requires_internet() -> Result<()> {
        let file = "../rv-lockfile/tests/inputs/Gemfile.lock.discourse".into();
        let cache = rv_cache::Cache::temp().unwrap();
        ci_inner(
            file,
            &cache,
            &CiArgs {
                max_concurrent_requests: 10,
                validate_checksums: true,
            },
        )
        .await?;
        Ok(())
    }
}
