use bytes::Bytes;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use current_platform::CURRENT_PLATFORM;
use dircpy::copy_dir;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use glob::glob;
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use rayon::ThreadPoolBuildError;
use regex::Regex;
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

use crate::commands::ruby::run::CaptureOutput;
use crate::config::Config;
use std::collections::HashMap;
use std::env::current_dir;
use std::io;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::ops::Not;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::str::FromStr;
use std::vec;

const ARM_STRINGS: [&str; 3] = ["arm64", "arm", "aarch64"];
const X86_STRINGS: [&str; 4] = ["x86", "i686", "win32", "win64"];

#[derive(Debug, clap_derive::Args)]
pub struct CleanInstallArgs {
    /// Path to Gemfile
    #[arg(long, env = "BUNDLE_GEMFILE")]
    gemfile: Option<Utf8PathBuf>,

    /// Maximum number of downloads that can be in flight at once.
    #[arg(short, long, default_value = "10")]
    pub max_concurrent_requests: usize,

    /// Maximum number of gem installations that can be in flight at once.
    /// This reduces concurrently-open files on your filesystem,
    /// and concurrent disk operations.
    #[arg(long, default_value = "20")]
    pub max_concurrent_installs: usize,

    /// Validate the checksums from the gem server and gem itself.
    #[arg(long, default_value = "true")]
    pub validate_checksums: bool,

    /// Don't compile the extensions in native gems.
    #[arg(long, default_value = "false")]
    pub skip_compile_extensions: bool,
}

#[derive(Debug)]
struct CiInnerArgs {
    pub skip_compile_extensions: bool,
    pub max_concurrent_requests: usize,
    pub max_concurrent_installs: usize,
    pub validate_checksums: bool,
    pub lockfile_path: Utf8PathBuf,
    pub install_path: Utf8PathBuf,
    pub exts_dir: Utf8PathBuf,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
    #[error("Cannot build unknown native extension {filename} from gem {gemname}")]
    UnknownExtension { filename: String, gemname: String },
    #[error("Some gems did not compile their extensions")]
    CompileFailures,
    #[error(transparent)]
    Config(#[from] crate::config::Error),
    #[error(transparent)]
    Run(#[from] crate::commands::ruby::run::Error),
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
    InvalidPath(PathBuf),
    #[error("Checksum file was not valid YAML")]
    InvalidChecksum,
    #[error("Gem {gem_name} archive did not include metadata.gz")]
    NoMetadata { gem_name: String },
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
    #[error("Could not write binstub for {dep_name}/{exe_name}: {error}")]
    CouldNotWriteBinstub {
        dep_name: String,
        exe_name: String,
        error: io::Error,
    },
    #[error("Could not download a git dependency: {error}")]
    Git { error: String },
    #[error(
        "The lockfile path must be inside a directory with a parent, but it wasn't. Path was {0}"
    )]
    InvalidLockfilePath(String),
}

type Result<T> = std::result::Result<T, Error>;

pub async fn ci(config: &Config, args: CleanInstallArgs) -> Result<()> {
    let lockfile_path = find_lockfile_path(args.gemfile)?;
    let lockfile_path_parent = lockfile_path
        .parent()
        .ok_or(Error::InvalidLockfilePath(lockfile_path.to_string()))?
        .to_path_buf();
    let install_path = find_install_path(config, &lockfile_path_parent).await?;
    let exts_dir = exts_dir(config)?;
    let inner_args = CiInnerArgs {
        skip_compile_extensions: args.skip_compile_extensions,
        max_concurrent_requests: args.max_concurrent_requests,
        max_concurrent_installs: args.max_concurrent_installs,
        validate_checksums: args.validate_checksums,
        lockfile_path,
        install_path,
        exts_dir,
    };
    ci_inner(config, &inner_args).await
}

async fn ci_inner(config: &Config, args: &CiInnerArgs) -> Result<()> {
    let lockfile_contents = tokio::fs::read_to_string(&args.lockfile_path).await?;
    let lockfile = rv_lockfile::parse(&lockfile_contents)?;

    if lockfile.path.is_empty().not() {
        tracing::warn!("rv ci does not support path deps yet");
    }

    let repos = download_git_repos(lockfile.clone(), &config.cache, args).await?;
    install_git_repos(config, repos, args).await?;

    let gems = download_gems(lockfile.clone(), &config.cache, args).await?;
    let specs = install_gems(config, gems, args).await?;
    compile_gems(config, specs, args).await?;

    Ok(())
}

async fn install_git_repos<'i>(
    config: &Config,
    repos: Vec<DownloadedGitRepo<'i>>,
    args: &CiInnerArgs,
) -> Result<()> {
    let git_gems_dir = args.install_path.join("bundler/gems");

    use rayon::prelude::*;
    let pool = create_rayon_pool(args.max_concurrent_installs).unwrap();
    pool.install(|| {
        repos
            .iter()
            .par_bridge()
            .map(|repo| install_git_repo(repo, &git_gems_dir, config, args))
            .collect::<Result<Vec<_>>>()?;
        Ok::<_, Error>(())
    })?;
    Ok(())
}

fn install_git_repo(
    repo: &DownloadedGitRepo,
    git_gems_dir: &Utf8Path,
    config: &Config,
    args: &CiInnerArgs,
) -> Result<()> {
    let repo_path = Utf8PathBuf::from(&repo.remote);
    let repo_name = repo_path.file_name().expect("repo has no filename?");
    let repo_name = repo_name.strip_suffix(".git").unwrap_or(repo_name);
    let git_name = format!("{}-{:.12}", repo_name, repo.sha);
    let dest_dir = git_gems_dir.join(git_name);
    let mut just_cloned = false;

    if std::fs::exists(&dest_dir)?.not() {
        tracing::event!(tracing::Level::DEBUG, %repo_path, %dest_dir, "Cloning from cached repo");
        let git_cloned = std::process::Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--no-checkout",
                repo_path.as_ref(),
                dest_dir.as_ref(),
            ])
            .spawn()?
            .wait()?;
        if !git_cloned.success() {
            return Err(Error::Git {
                error: format!("git clone had exit code {}", git_cloned),
            });
        }
        just_cloned = true
    }

    if !just_cloned {
        tracing::event!(tracing::Level::DEBUG, %repo_path, %dest_dir, "Fetching from cached repo");
        let git_cloned = std::process::Command::new("git")
            .current_dir(&dest_dir)
            .args(["fetch", "--quiet", "--force", "--tags", dest_dir.as_ref()])
            .spawn()?
            .wait()?;
        if !git_cloned.success() {
            return Err(Error::Git {
                error: format!("git fetch had exit code {}", git_cloned),
            });
        }
    }

    tracing::event!(tracing::Level::DEBUG, %repo_path, %dest_dir, %repo.sha, "resetting to the locked sha");
    let git_cloned = std::process::Command::new("git")
        .current_dir(&dest_dir)
        .args(["reset", "--quiet", "--hard", &repo.sha])
        .spawn()?
        .wait()?;
    if !git_cloned.success() {
        return Err(Error::Git {
            error: format!("git reset had exit code {}", git_cloned),
        });
    }

    // TODO: if the git source in the lockfile has submodules: true, then we should
    //   "git", "submodule", "update", "--quiet", "--init", "--recursive"

    debug!("Installed repo {}", &repo_name);

    let cached_gemspecs_dir = config
        .cache
        .shard(rv_cache::CacheBucket::Gemspec, "gemspecs")
        .into_path_buf();
    fs_err::create_dir_all(&cached_gemspecs_dir)?;

    let pattern = dest_dir.join("**/*.gemspec").to_string();
    for path in glob(&pattern).expect("invalid glob pattern").flatten() {
        debug!("found gemspec at {:?}", path);
        // find the .gemspec file(s)
        let gitsha = &repo.sha;
        let cached_gemspecs_dir = config
            .cache
            .shard(rv_cache::CacheBucket::Gemspec, "gemspecs")
            .into_path_buf();
        let dep = repo.specs.iter().find(|s| {
            path.to_string_lossy()
                .contains(&format!("{}.gemspec", s.gem_version.name))
        });
        if let Some(dep) = dep {
            // check the cache for "gitsha-gemname.gemspec", if not:
            let gemname = dep.gem_version;
            let cache_key = format!("{gitsha}-{gemname}.gemspec");
            let cached_gemspec_path = cached_gemspecs_dir.join(&cache_key);
            // let cached = std::fs::exists(&cached_gemspec_path).is_ok_and(|exists| exists);
            let cached = false;
            if !cached {
                // shell out to ruby -e 'puts Gem::Specification.load("name.gemspec").to_yaml' to get the YAML-format gemspec as a string
                let yaml_gemspec_vec = crate::commands::ruby::run::run_no_install(
                    config,
                    &config.ruby_request().unwrap(), // TODO: remove the unwrap
                    &[
                        "-e",
                        &format!(
                            "puts Gem::Specification.load(\"{}\").to_yaml",
                            path.to_string_lossy() // TODO: how do I interpolate an os_str into a shell arg :(
                        ),
                    ],
                    CaptureOutput::Both,
                    Some(&repo.path),
                    Vec::new(),
                )?
                .stdout;
                let yaml_gemspec = String::try_from(yaml_gemspec_vec).unwrap(); // arghhhhhhh
                debug!("evaluated gemspec into YAML:\n{}", yaml_gemspec);
                // cache the YAML gemspec as "gitsha-gemname.gemspec"
                fs_err::write(&cached_gemspec_path, yaml_gemspec)?;
            }
            // parse the YAML gemspec to get the executable names
            let yaml_contents = std::fs::read_to_string(cached_gemspec_path)?;
            let dep_gemspec = match rv_gem_specification_yaml::parse(&yaml_contents) {
                Ok(parsed) => parsed,
                Err(e) => {
                    eprintln!("Warning: specification of {gemname} was invalid: {e}");
                    return Ok(());
                }
            };
            // pass the executable names to the existing binstub generation code
            let binstub_dir = args.install_path.join("bin");
            install_binstub(&dep_gemspec.name, &dep_gemspec.executables, &binstub_dir)?;
        }
    }

    Ok(())
}

async fn download_git_repos<'i>(
    lockfile: GemfileDotLock<'i>,
    cache: &rv_cache::Cache,
    _args: &CiInnerArgs,
) -> Result<Vec<DownloadedGitRepo<'i>>> {
    let num_specs: usize = lockfile.git.iter().map(|source| source.specs.len()).sum();
    let mut downloads = Vec::with_capacity(num_specs);

    // Download git repos to this dir.
    let git_clone_dir = cache
        .shard(rv_cache::CacheBucket::Git, "gits")
        .into_path_buf();
    fs_err::create_dir_all(&git_clone_dir)?;

    for git_source in lockfile.git {
        // This will be the subdir within `git_clone_dir` that the git cloned repos are written to.
        let cache_key = rv_cache::cache_digest((git_source.remote, git_source.revision));
        let git_repo_dir = git_clone_dir.clone().join(&cache_key);

        if std::fs::exists(&git_repo_dir)? {
            tracing::event!(tracing::Level::DEBUG, %git_repo_dir, %git_source.remote, %git_source.revision, "checking for revision");
            let sha_check = std::process::Command::new("git")
                .current_dir(&git_repo_dir)
                .args([
                    "--no-lazy-fetch",
                    "cat-file",
                    "-e",
                    &format!("{}^{{commit}}", git_source.revision),
                ])
                .spawn()?
                .wait()?;
            if !sha_check.success() {
                tracing::event!(tracing::Level::DEBUG, %git_repo_dir, %git_source.remote, %git_source.revision, "updating repo");
                let git_fetch = std::process::Command::new("git")
                    .current_dir(&git_repo_dir)
                    .args([
                        "fetch",
                        "--quiet",
                        "--force",
                        "--tags",
                        git_source.remote,
                        "refs/heads/*:refs/heads/*",
                    ])
                    .spawn()?
                    .wait()?;
                if !git_fetch.success() {
                    return Err(Error::Git {
                        error: format!("git fetch had exit code {}", git_fetch),
                    });
                }
            }
        } else {
            tracing::event!(tracing::Level::DEBUG, %git_clone_dir, %git_source.remote, %git_source.revision, "Cloning repo");
            let git_cloned = std::process::Command::new("git")
                .current_dir(&git_clone_dir)
                .args([
                    "clone",
                    "--quiet",
                    "--bare",
                    "--no-hardlinks",
                    git_source.remote,
                    cache_key.as_ref(),
                ])
                .spawn()?
                .wait()?;
            if !git_cloned.success() {
                return Err(Error::Git {
                    error: format!("git clone had exit code {}", git_cloned),
                });
            }
        }

        // Success! Save the paths of all the repos we just cloned.
        downloads.push(DownloadedGitRepo {
            remote: git_source.remote.to_string(),
            specs: git_source.specs.clone(),
            sha: git_source.revision.to_string(),
            path: git_repo_dir.clone(),
        });
    }
    Ok(downloads)
}

fn find_lockfile_path(gemfile: Option<Utf8PathBuf>) -> Result<Utf8PathBuf> {
    let lockfile_name: Utf8PathBuf;
    if let Some(path) = gemfile {
        lockfile_name = format!("{}.lock", path).into();
    } else {
        lockfile_name = "Gemfile.lock".into();
    }
    let lockfile_path = match Utf8PathBuf::from_path_buf(current_dir()?.join(lockfile_name)) {
        Ok(it) => it,
        Err(err) => return Err(Error::InvalidPath(err)),
    };
    debug!("found lockfile_path {}", lockfile_path);
    Ok(lockfile_path)
}

/// Which path should `ci` install gems under?
/// Uses Bundler's `bundle_path`.
async fn find_install_path(config: &Config, lockfile_dir: &Utf8Path) -> Result<Utf8PathBuf> {
    let env_path = std::env::var("BUNDLE_PATH");
    if let Ok(bundle_path) = env_path {
        return Ok(Utf8PathBuf::from(&bundle_path));
    }
    let args = ["-rbundler", "-e", "puts Bundler.bundle_path"];
    let bundle_path = crate::commands::ruby::run::run(
        config,
        None,
        Default::default(),
        args.as_slice(),
        CaptureOutput::Both,
        Some(lockfile_dir),
    )
    .await?
    .stdout;

    if bundle_path.is_empty() {
        return Err(Error::BadBundlePath);
    }
    let bundle_path = String::from_utf8(bundle_path)
        .map(|s| Utf8PathBuf::from(s.trim()))
        .map_err(|_| Error::BadBundlePath);
    debug!("found install path {:?}", bundle_path);
    bundle_path
}

pub fn create_rayon_pool(
    num_threads: usize,
) -> std::result::Result<rayon::ThreadPool, ThreadPoolBuildError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
}

async fn install_gems<'i>(
    _config: &Config,
    downloaded: Vec<DownloadedRubygems<'i>>,
    args: &CiInnerArgs,
) -> Result<Vec<GemSpecification>> {
    let binstub_dir = args.install_path.join("bin");
    debug!("about to create {}", binstub_dir);
    tokio::fs::create_dir_all(&binstub_dir).await?;
    debug!("finished creating {}", binstub_dir);
    use rayon::prelude::*;
    let pool = create_rayon_pool(args.max_concurrent_installs).unwrap();
    let specs = pool.install(|| {
        downloaded
            .into_iter()
            .par_bridge()
            .map(|download| {
                let gv = download.spec.gem_version;
                // Actually unpack the tarball here.
                let dep_gemspec_res = download.unpack_tarball(args.install_path.clone(), args)?;
                let Some(dep_gemspec) = dep_gemspec_res else {
                    panic!(
                        "Warning: No gemspec found for downloaded gem {}",
                        gv.yellow()
                    );
                };
                install_binstub(&dep_gemspec.name, &dep_gemspec.executables, &binstub_dir)?;
                debug!("Unpacked {gv}");
                Ok(dep_gemspec)
            })
            .collect::<Result<Vec<GemSpecification>>>()
    })?;

    Ok(specs)
}

async fn compile_gems(
    config: &Config,
    specs: Vec<GemSpecification>,
    args: &CiInnerArgs,
) -> Result<()> {
    use rayon::prelude::*;
    let pool = create_rayon_pool(args.max_concurrent_installs).unwrap();
    pool.install(|| {
        specs.into_iter().par_bridge().map(|spec| {
            // 4. Handle compiling native extensions for gems with native extensions
            if !args.skip_compile_extensions && !spec.extensions.is_empty() {
                debug!("compiling native extensions for {}", spec.full_name());
                let compiled_ok = compile_gem(config, args, spec)?;
                if !compiled_ok {
                    return Err(Error::CompileFailures);
                }
            }
            Ok(())
        })
    })
    .collect::<Result<()>>()?;
    Ok(())
}

fn install_binstub(dep_name: &str, executables: &[String], binstub_dir: &Utf8Path) -> Result<()> {
    for exe_name in executables {
        if let Err(error) = write_binstub(dep_name, exe_name, binstub_dir) {
            return Err(Error::CouldNotWriteBinstub {
                dep_name: dep_name.to_owned(),
                exe_name: exe_name.to_owned(),
                error,
            });
        }
    }
    Ok(())
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

/// Downloads all Rubygem server gems from a Gemfile.lock
async fn download_gems<'i>(
    lockfile: GemfileDotLock<'i>,
    cache: &rv_cache::Cache,
    args: &CiInnerArgs,
) -> Result<Vec<DownloadedRubygems<'i>>> {
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
    debug!("Downloaded all gems");
    Ok(downloaded)
}

/// A gem downloaded from a RubyGems source.
struct DownloadedRubygems<'i> {
    contents: Bytes,
    spec: Spec<'i>,
}

/// A gem downloaded from a git source.
#[derive(Debug)]
struct DownloadedGitRepo<'i> {
    remote: String,
    path: Utf8PathBuf,
    specs: Vec<Spec<'i>>,
    sha: String,
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
        let root = contents_yaml.first()?;
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

impl<'i> DownloadedRubygems<'i> {
    fn unpack_tarball(
        self,
        bundle_path: Utf8PathBuf,
        args: &CiInnerArgs,
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

        // If the user wants to validate checksums,
        // then iterate through the archive entries until you find the checksum entry.
        // We'll then store it, and iterate through the archive a second time to find
        // the real files, and validate their checksums.
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

                    // Should not happen in practice, because we break after finding the checksums.
                    // But may as well be defensive here.
                    if checksums.replace(cs).is_some() {
                        return Err(Error::InvalidGemArchive(
                            "two checksums.yaml.gz files found in the gem archive".to_owned(),
                        ));
                    }
                    break;
                }
            }
            if checksums.is_none() {
                // eprintln!(
                //     "Warning: No checksums found for gem {}",
                //     nameversion.yellow()
                // );
            }
            checksums
        } else {
            None
        };

        // Now that we've handled checksums (perhaps), we can iterate through the archive
        // and unpack the entries we care about. Specifically the metadata and the data itself.
        // If we found checksums, validate them.
        let mut found_gemspec = None;
        let mut found_data_tar = false;
        let mut archive = tar::Archive::new(contents);
        for e in archive.entries()? {
            let entry = e?;
            let entry_path = entry.path()?;
            match entry_path.display().to_string().as_str() {
                "metadata.gz" => {
                    // Unpack the metadata, which stores the gem specs.
                    if found_gemspec.is_some() {
                        return Err(Error::InvalidGemArchive("two metadata.gz found".to_owned()));
                    }
                    let UnpackedMetdata { hashed, gemspec } =
                        unpack_metadata(&bundle_path, &nameversion, HashReader::new(entry))?;
                    found_gemspec = Some(gemspec);
                    if args.validate_checksums
                        && let Some(ref checksums) = checksums
                    {
                        checksums.validate_metadata(nameversion.clone(), hashed)?
                    }
                }
                "data.tar.gz" => {
                    // Unpack the data archive, which stores all the gems.
                    if found_data_tar {
                        return Err(Error::InvalidGemArchive("two data.tar.gz found".to_owned()));
                    }
                    let unpacked =
                        unpack_data_tar(&bundle_path, &nameversion, HashReader::new(entry))?;
                    if args.validate_checksums
                        && let Some(ref checksums) = checksums
                    {
                        checksums.validate_data_tar(nameversion.clone(), &unpacked.hashed)?
                    }
                    found_data_tar = true;
                }
                "checksums.yaml.gz" => {
                    // Already handled in the earlier loop above.
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

        if !found_data_tar {
            return Err(Error::NoDataTar);
        }
        let Some(found_gemspec) = found_gemspec else {
            return Err(Error::NoMetadata {
                gem_name: nameversion,
            });
        };

        Ok(found_gemspec)
    }
}

fn generate_binstub_contents(gem_name: &str, exe_name: &str) -> String {
    format!(
        r#"#!/usr/bin/env ruby
# This executable comes from the '{gem_name}' gem, generated by https://rv.dev

require 'rubygems'

Gem.use_gemdeps

version = ">= 0.a"

str = ARGV.first
if str
  str = str.b[/\A_(.*)_\z/, 1]
  if str and Gem::Version.correct?(str)
    version = str
    ARGV.shift
  end
end

if Gem.respond_to?(:activate_and_load_bin_path)
  Gem.activate_and_load_bin_path('{gem_name}', '{exe_name}', version)
else
  load Gem.activate_bin_path('{gem_name}', '{exe_name}', version)
end"#
    )
}

fn write_binstub(gem_name: &str, exe_name: &str, binstub_dir: &Utf8Path) -> io::Result<()> {
    let binstub_path = binstub_dir.join(exe_name);
    let binstub_contents = generate_binstub_contents(gem_name, exe_name);
    fs_err::write(binstub_path, binstub_contents)
}

struct CompileNativeExtResult {
    extension: String,
    outputs: Vec<std::process::Output>,
}

impl CompileNativeExtResult {
    pub fn success(&self) -> bool {
        self.outputs.iter().all(|o| o.status.success())
    }
}

fn exts_dir(config: &Config) -> Result<Utf8PathBuf> {
    let exts_dir = crate::commands::ruby::run::run_no_install(
        config,
        &config.ruby_request()?,
        &[
            "-e",
            "puts File.join(Gem::Platform.local.to_s, Gem.extension_api_version)",
        ],
        CaptureOutput::Both,
        None,
        vec![],
    )?
    .stdout;

    String::from_utf8(exts_dir)
        .map(|s| Utf8PathBuf::from(s.trim()))
        .map_err(|_| Error::BadBundlePath)
}

static EXTCONF_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)extconf").unwrap());
static RAKE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)rakefile|mkrf_conf").unwrap());

fn compile_gem(config: &Config, args: &CiInnerArgs, spec: GemSpecification) -> Result<bool> {
    let mut compile_results = Vec::with_capacity(spec.extensions.len());

    let gem_home = &args.install_path;
    let gem_path = gem_home.join("gems").join(spec.full_name());
    let lib_dest = gem_path.join("lib");
    let ext_dest = args
        .install_path
        .join("extensions")
        .join(&args.exts_dir)
        .join(spec.full_name());
    let mut ran_rake = false;

    if std::fs::exists(ext_dest.join("gem.build_complete"))? {
        return Ok(true);
    }

    for extstr in spec.extensions.clone() {
        let extension = extstr.as_ref();
        if EXTCONF_REGEX.is_match(extension) {
            if let Ok(outputs) =
                build_extconf(config, extension, gem_home, &gem_path, &ext_dest, &lib_dest)
            {
                compile_results.push(CompileNativeExtResult {
                    extension: extension.to_string(),
                    outputs,
                });
            }
        } else if RAKE_REGEX.is_match(extension) {
            if !ran_rake
                && let Ok(outputs) =
                    build_rakefile(config, extension, &gem_path, &ext_dest, &lib_dest)
            {
                compile_results.push(CompileNativeExtResult {
                    extension: extension.to_string(),
                    outputs,
                });
            }
            // Ensure that we only run the Rake builder once, even if we have both a `Rakefile` and `mkrf_conf` file
            ran_rake = true;
        } else {
            return Err(Error::UnknownExtension {
                filename: extension.to_string(),
                gemname: spec.full_name(),
            });
        }
    }

    fs_err::create_dir_all(&ext_dest)?;
    let mut log = fs_err::File::create(ext_dest.join("build_ext.log"))?;
    for res in compile_results.iter() {
        for out in res.outputs.iter() {
            log.write_all(&out.stdout)?;
            log.write_all(&out.stderr)?;
            log.write_all(b"\n\n")?;
        }

        if res.success() {
            continue;
        }

        for out in res.outputs.iter() {
            eprintln!(
                "Warning: Could not compile gem {}'s extension {}. Got exit code {}.",
                spec.full_name().yellow(),
                res.extension.yellow(),
                out.status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or("<unknown>".to_owned()),
            );
            if !out.stdout.is_empty() {
                eprintln!("stdout was:\n{}", String::from_utf8_lossy(&out.stdout));
            }
            if !out.stderr.is_empty() {
                eprintln!("stderr was:\n{}", String::from_utf8_lossy(&out.stderr));
            }
        }
    }

    let all_ok = compile_results.iter().all(|res| res.success());
    Ok(all_ok)
}

fn build_rakefile(
    config: &Config,
    extension: &str,
    gem_path: &Utf8PathBuf,
    ext_dest: &Utf8PathBuf,
    lib_dest: &Utf8PathBuf,
) -> Result<Vec<std::process::Output>> {
    let ext_path = Utf8PathBuf::from_str(extension)?;
    let ext_dir = gem_path.join(ext_path.parent().expect("extconf has no parent"));
    let ext_file = ext_path.file_name().expect("extconf has no filename");
    let mut output;
    let mut outputs = vec![];

    // 1. Run mkrf if needed to create the Rakefile
    if ext_file.to_lowercase().contains("mkrf_conf") {
        output = crate::commands::ruby::run::run_no_install(
            config,
            &config.ruby_request()?,
            &[ext_file],
            CaptureOutput::Both,
            Some(&ext_dir),
            vec![],
        )?;
        outputs.push(output);
    }

    // 2. Run Rake with the args
    let tmp_dir = camino_tempfile::tempdir_in(gem_path)?;
    let sitearchdir = format!("RUBYARCHDIR={}", tmp_dir.path());
    let sitelibdir = format!("RUBYLIBDIR={}", tmp_dir.path());
    let args = vec![sitearchdir, sitelibdir];
    output = Command::new("rake")
        .args(&args)
        .current_dir(&ext_dir)
        .output()?;
    outputs.push(output);

    // 3. Copy the resulting files to ext and lib dirs
    copy_dir(&tmp_dir, lib_dest)?;
    copy_dir(&tmp_dir, ext_dest)?;

    // 4. Mark the gem as built
    fs_err::write(ext_dest.join("gem.build_complete"), "")?;

    Ok(outputs)
}

fn build_extconf(
    config: &Config,
    extension: &str,
    gem_home: &Utf8PathBuf,
    gem_path: &Utf8PathBuf,
    ext_dest: &Utf8PathBuf,
    lib_dest: &Utf8PathBuf,
) -> Result<Vec<std::process::Output>> {
    let ext_path = Utf8PathBuf::from_str(extension)?;
    let ext_dir = gem_path.join(ext_path.parent().expect("extconf has no parent"));
    let ext_file = ext_path.file_name().expect("extconf has no filename");
    let mut output;
    let mut outputs = vec![];

    // 1. Run the extconf.rb file with the current ruby
    output = crate::commands::ruby::run::run_no_install(
        config,
        &config.ruby_request()?,
        &[ext_file],
        CaptureOutput::Both,
        Some(&ext_dir),
        vec![("GEM_HOME", gem_home.as_str())],
    )?;
    outputs.push(output);

    // 2. Save the mkmf.log file if it exists
    let mkmf_log = ext_dir.join("mkmf.log");
    if mkmf_log.exists() {
        fs_err::create_dir_all(ext_dest)?;
        fs_err::rename(mkmf_log, ext_dest.join("mkmf.log"))?;
    }

    // 3. Run make clean / make / make install / make clean
    let tmp_dir = camino_tempfile::tempdir_in(gem_path)?;
    let sitearchdir = format!("sitearchdir={}", tmp_dir.path());
    let sitelibdir = format!("sitelibdir={}", tmp_dir.path());
    let args = vec!["DESTDIR=''", &sitearchdir, &sitelibdir];

    Command::new("make")
        .args([vec!["clean"], args.clone()].concat())
        .current_dir(&ext_dir)
        .output()?;

    output = Command::new("make")
        .args(&args)
        .current_dir(&ext_dir)
        .output()?;
    let success = output.status.success();
    outputs.push(output);
    if !success {
        return Ok(outputs);
    }

    output = Command::new("make")
        .args([vec!["install"], args.clone()].concat())
        .current_dir(&ext_dir)
        .output()?;
    outputs.push(output);

    Command::new("make")
        .args([vec!["clean"], args.clone()].concat())
        .current_dir(&ext_dir)
        .output()?;

    // 4. Copy the resulting files to ext and lib dirs
    copy_dir(&tmp_dir, lib_dest)?;
    copy_dir(&tmp_dir, ext_dest)?;

    // 5. Mark the gem as built
    fs_err::write(ext_dest.join("gem.build_complete"), "")?;

    Ok(outputs)
}

/// Wrapper around some reader type `R`
/// that also computes SHA checksums as it reads.
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
    /// Wrap the `reader` into this `HashReader` which both
    /// reads and also computes checksums.
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
}

/// Given the data.tar.gz from a gem, unpack its contents to the filesystem under
/// BUNDLEPATH/gems/name-version/ENTRY
/// Returns the checksum.
fn unpack_data_tar<R>(
    bundle_path: &Utf8Path,
    nameversion: &str,
    data_tar_gz: HashReader<R>,
) -> Result<UnpackedData>
where
    R: std::io::Read,
{
    // First, create the data's destination.
    let data_dir: PathBuf = bundle_path.join("gems").join(nameversion).into();
    fs_err::create_dir_all(&data_dir)?;
    let mut gem_data_archive = tar::Archive::new(GzDecoder::new(data_tar_gz));
    for e in gem_data_archive.entries()? {
        let mut entry = e?;
        let entry_path = entry.path()?;
        let dst = data_dir.join(entry_path);

        // Not sure if this is strictly necessary, or if we can know the
        // intermediate directories ahead of time.
        if let Some(dst_parent) = dst.parent() {
            fs_err::create_dir_all(dst_parent)?;
        }
        entry.unpack(&dst)?;
    }
    // Get the HashReader back.
    let h = gem_data_archive.into_inner().into_inner();
    let hashed = h.finalize();
    Ok(UnpackedData { hashed })
}

struct UnpackedMetdata {
    hashed: Hashed,
    gemspec: Option<GemSpecification>,
}

/// Given the metadata.gz from a gem, write it to the filesystem under
/// BUNDLEPATH/specifications/name-version.gemspec
fn unpack_metadata<R>(
    bundle_path: &Utf8Path,
    nameversion: &str,
    metadata_gz: HashReader<R>,
) -> Result<UnpackedMetdata>
where
    R: Read,
{
    // First, create the metadata's destination.
    let metadata_dir = bundle_path.join("specifications/");
    fs_err::create_dir_all(&metadata_dir)?;
    let filename = format!("{nameversion}.gemspec");
    let dst_path = metadata_dir.join(filename);
    let mut dst = fs_err::File::create(&dst_path)?;

    // Then write the (unzipped) source into the destination.
    let mut yaml_contents = String::new();
    let mut unzipper = GzDecoder::new(metadata_gz);
    unzipper.read_to_string(&mut yaml_contents)?;
    let parsed = match rv_gem_specification_yaml::parse(&yaml_contents) {
        Ok(parsed) => Some(parsed),
        Err(e) => {
            eprintln!("Warning: specification of {nameversion} was invalid: {e}");
            None
        }
    };
    let ruby_contents = convert_gemspec_yaml_to_ruby(yaml_contents);
    std::io::copy(&mut Cursor::new(ruby_contents), &mut dst)?;

    let h = unzipper.into_inner();
    Ok(UnpackedMetdata {
        hashed: h.finalize(),
        gemspec: parsed,
    })
}

fn convert_gemspec_yaml_to_ruby(contents: String) -> String {
    let mut child = std::process::Command::new("ruby")
        .args([
            "-e",
            "Gem.load_yaml; print Gem::SafeYAML.safe_load(ARGF.read).to_ruby",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn child process");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    std::thread::spawn(move || {
        stdin
            .write_all(contents.as_bytes())
            .expect("Failed to write to stdin");
    });

    let output = child.wait_with_output().expect("Failed to read stdout");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn platform_for_gem(gem_version: &str) -> Platform {
    let is_arm = ARM_STRINGS.iter().any(|s| gem_version.contains(s));
    let is_x86 = X86_STRINGS.iter().any(|s| gem_version.contains(s));
    // Comments starting with `when` are relevant examples from rubygems.
    let (cpu, os) = if gem_version.contains("darwin") {
        //        when /^i686-darwin(\d)/     then ["x86",       "darwin",  $1]
        //        when "powerpc-darwin"       then ["powerpc",   "darwin",  nil]
        //        when /powerpc-darwin(\d)/   then ["powerpc",   "darwin",  $1]
        //        when /universal-darwin(\d)/ then ["universal", "darwin",  $1]
        if is_x86 {
            (Cpu::X86, Os::Darwin)
        } else if gem_version.contains("powerpc") {
            (Cpu::Powerpc, Os::Darwin)
        } else if gem_version.contains("universal") {
            (Cpu::Universal, Os::Darwin)
        } else if is_arm {
            (Cpu::Arm, Os::Darwin)
        } else {
            (Cpu::Unknown, Os::Darwin)
        }
    } else if gem_version.contains("linux") {
        // when /^i\d86-linux/         then ["x86",       "linux",   nil]
        if is_x86 {
            (Cpu::X86, Os::Linux)
        } else if is_arm {
            (Cpu::Arm, Os::Linux)
        } else {
            (Cpu::Unknown, Os::Linux)
        }
    } else if gem_version.contains("sparc-solaris") {
        // when /sparc-solaris2.8/     then ["sparc",     "solaris", "2.8"]
        if gem_version.contains("sparc") {
            (Cpu::Sparc, Os::Solaris)
        } else {
            (Cpu::Unknown, Os::Solaris)
        }
    } else if gem_version.contains("mswin") {
        // when /mswin32(\_(\d+))?/    then ["x86",       "mswin32", $2]
        // when /mswin64(\_(\d+))?/    then ["x64",       "mswin64", $2]
        if is_x86 {
            (Cpu::X86, Os::Windows)
        } else if is_arm {
            (Cpu::Arm, Os::Windows)
        } else {
            (Cpu::Unknown, Os::Windows)
        }
    } else if gem_version.contains("mingw") {
        if gem_version.contains("32") || gem_version.contains("64") {
            (Cpu::X86, Os::Mingw)
        } else if is_arm {
            (Cpu::Arm, Os::Mingw)
        } else {
            (Cpu::Unknown, Os::Mingw)
        }
    } else if gem_version.contains("java") || gem_version.contains("jruby") {
        // when "java", "jruby"        then [nil,         "java",    nil]
        (Cpu::Unknown, Os::Java)
    } else if gem_version.contains("dalvik") {
        // when /^dalvik(\d+)?$/       then [nil,         "dalvik",  $1]
        (Cpu::Unknown, Os::Dalvik)
    } else if gem_version.contains("dotnet") {
        // when /dotnet(\-(\d+\.\d+))? then ["universal", "dotnet",  $2]
        (Cpu::Universal, Os::Dotnet)
    } else {
        (Cpu::Unknown, Os::Unknown)
    };
    Platform { cpu, os }
}

#[derive(Debug, Eq, PartialEq)]
enum Cpu {
    X86,
    Arm,
    Powerpc,
    Unknown,
    Universal,
    Sparc,
}

impl Cpu {
    fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            // Err on the side of caution for unknown.
            // And universal means it should match everything, right?
            (Self::Universal, _)
            | (Self::Unknown, _)
            | (_, Self::Universal)
            | (_, Self::Unknown) => true,
            // Other types should be matched exactly, i.e. be the same enum variant.
            (a, b) => a == b,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Os {
    Darwin,
    Windows,
    Mingw,
    Linux,
    Solaris,
    Java,
    Dalvik,
    Dotnet,
    Unknown,
}

impl Os {
    fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            // Err on the side of caution for unknown.
            (Self::Unknown, _) | (_, Self::Unknown) => true,
            // Other types should be matched exactly, i.e. be the same enum variant.
            (a, b) => a == b,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Platform {
    cpu: Cpu,
    os: Os,
}

impl Platform {
    fn matches(&self, other: &Self) -> bool {
        self.cpu.matches(&other.cpu) && self.os.matches(&other.os)
    }
}

impl Platform {
    fn current() -> Self {
        match CURRENT_PLATFORM {
            "aarch64-apple-darwin" => Platform {
                os: Os::Darwin,
                cpu: Cpu::Arm,
            },
            "x86_64-apple-darwin" => Platform {
                os: Os::Darwin,
                cpu: Cpu::X86,
            },
            "x86_64-unknown-linux-gnu" => Platform {
                os: Os::Linux,
                cpu: Cpu::X86,
            },
            "aarch64-unknown-linux-gnu" => Platform {
                os: Os::Linux,
                cpu: Cpu::Arm,
            },
            other => {
                #[cfg(debug_assertions)]
                {
                    panic!(
                        "Unknown target {}, please add it to the above match stmt",
                        other
                    );
                }
                #[cfg(not(debug_assertions))]
                {
                    eprintln!("Warning: unknown OS {}", other.yellow());
                    Platform {
                        os: Os::Unknown,
                        cpu: Cpu::Unknown,
                    }
                }
            }
        }
    }
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
) -> Result<Vec<DownloadedRubygems<'i>>> {
    // TODO: If the gem server needs user credentials, accept them and add them to this client.
    let client = rv_http_client()?;

    // Download them all, concurrently.

    let gems_to_download = gem_source.specs.into_iter().filter(|spec| {
        let arch = platform_for_gem(spec.gem_version.version);
        let this_arch = Platform::current();
        arch.matches(&this_arch)
    });
    let spec_stream = futures_util::stream::iter(gems_to_download);
    let downloaded_gems: Vec<_> = spec_stream
        .map(|spec| download_gem(gem_source.remote, spec, &client, cache, checksums))
        .buffered(max_concurrent_requests)
        .try_collect()
        .await?;
    debug!(
        "Finished downloading gems from source {}",
        gem_source.remote
    );
    Ok(downloaded_gems)
}

/// Download a single gem, from the given URL, using the given client.
async fn download_gem<'i>(
    remote: &str,
    spec: Spec<'i>,
    client: &Client,
    cache: &rv_cache::Cache,
    checksums: &HashMap<GemVersion<'i>, HowToChecksum>,
) -> Result<DownloadedRubygems<'i>> {
    let url = url_for_spec(remote, &spec)?;
    let cache_key = rv_cache::cache_digest(url.as_ref());
    let cache_path = cache
        .shard(rv_cache::CacheBucket::Gem, "gems")
        .into_path_buf()
        .join(format!("{cache_key}.gem"));

    let contents = if cache_path.exists() {
        debug!("Reusing gem from {url} in cache");
        let data = tokio::fs::read(&cache_path).await?;
        Bytes::from(data)
    } else {
        debug!("Downloading gem from {url}");
        client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
    };

    // Validate the checksums.
    if let Some(checksum) = checksums.get(&spec.gem_version) {
        match checksum.algorithm {
            KnownChecksumAlgos::Sha256 => {
                let actual = sha2::Sha256::digest(&contents);
                if actual[..] != checksum.value {
                    return Err(Error::ChecksumFail {
                        filename: url.to_string(),
                        gem_name: spec.gem_version.to_string(),
                        algo: "sha256",
                    });
                }
            }
        }
    }
    debug!("Validated {}", spec.gem_version);
    if !cache_path.exists() {
        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&cache_path, &contents).await?;
        debug!("Cached {}", spec.gem_version);
    }
    Ok(DownloadedRubygems { contents, spec })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_binstub() {
        let gem_name = "rake";
        let exe_name = "rake";
        let actual = generate_binstub_contents(gem_name, exe_name);
        let expected = r#"#!/usr/bin/env ruby
# This executable comes from the 'rake' gem, generated by https://rv.dev

require 'rubygems'

Gem.use_gemdeps

version = ">= 0.a"

str = ARGV.first
if str
  str = str.b[/\A_(.*)_\z/, 1]
  if str and Gem::Version.correct?(str)
    version = str
    ARGV.shift
  end
end

if Gem.respond_to?(:activate_and_load_bin_path)
  Gem.activate_and_load_bin_path('rake', 'rake', version)
else
  load Gem.activate_bin_path('rake', 'rake', version)
end"#;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_matches() {
        use Cpu::*;
        use Os::*;
        let should_match = [
            // They're the same.
            ((Arm, Darwin), (Arm, Darwin)),
            // Unknown should always match a known.
            ((Cpu::Unknown, Darwin), (Arm, Darwin)),
            ((Arm, Darwin), (Cpu::Unknown, Darwin)),
            ((Arm, Os::Unknown), (Arm, Darwin)),
            ((Arm, Darwin), (Arm, Os::Unknown)),
        ];
        for input in should_match {
            let p0 = Platform {
                cpu: input.0.0,
                os: input.0.1,
            };
            let p1 = Platform {
                cpu: input.1.0,
                os: input.1.1,
            };
            assert!(p0.matches(&p1));
        }
        let should_not_match = [
            // Different OS, same CPU
            ((Arm, Darwin), (Arm, Linux)),
            // Different CPU, same OS
            ((Arm, Linux), (X86, Linux)),
            // Different CPU and OS
            ((Arm, Darwin), (X86, Linux)),
        ];
        for input in should_not_match {
            let p0 = Platform {
                cpu: input.0.0,
                os: input.0.1,
            };
            let p1 = Platform {
                cpu: input.1.0,
                os: input.1.1,
            };
            assert!(!p0.matches(&p1));
        }
    }

    #[test]
    fn test_platform_for_gem() {
        use Cpu::*;
        use Os::*;
        for (gem_version, expected) in [
            // Real gem versions taken from Discourse's gemfile.lock
            ("1.17.2-arm64-darwin", (Arm, Darwin)),
            ("1.17.2-x86_64-darwin", (X86, Darwin)),
            ("2.7.4-x86_64-linux-gnu", (X86, Linux)),
            ("2.7.4-x86_64-linux-musl", (X86, Linux)),
            ("0.5.5-aarch64-linux-musl", (Arm, Linux)),
            ("2.7.4-aarch64-linux-gnu", (Arm, Linux)),
            ("1.17.2-arm-linux-gnu", (Arm, Linux)),
        ] {
            let actual = platform_for_gem(gem_version);
            let expected = Platform {
                cpu: expected.0,
                os: expected.1,
            };
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_platform_current() {
        use Cpu::*;
        use Os::*;
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let expected = Platform {
            os: Darwin,
            cpu: X86,
        };
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let expected = Platform {
            os: Darwin,
            cpu: Arm,
        };
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let expected = Platform {
            os: Linux,
            cpu: X86,
        };
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let expected = Platform {
            os: Linux,
            cpu: Arm,
        };
        let actual = Platform::current();
        assert_eq!(actual, expected);
    }
}
