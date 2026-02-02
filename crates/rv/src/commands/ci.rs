use bytes::Bytes;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use dircpy::copy_dir;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use glob::glob;
use indicatif::ProgressStyle;
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use rayon::ThreadPoolBuildError;
use regex::Regex;
use reqwest::Client;
use rv_gem_types::Specification as GemSpecification;
use rv_gem_types::VersionPlatform;
use rv_lockfile::datatypes::ChecksumAlgorithm;
use rv_lockfile::datatypes::GemSection;
use rv_lockfile::datatypes::GemVersion;
use rv_lockfile::datatypes::GemfileDotLock;
use rv_lockfile::datatypes::GitSection;
use rv_lockfile::datatypes::Spec;
use rv_ruby::request::RubyRequest;
use sha2::Digest;
use tracing::debug;
use tracing::info;
use tracing::info_span;
use tracing_indicatif::span_ext::IndicatifSpanExt;
use url::Url;

use crate::commands::ci::checksums::ArchiveChecksums;
use crate::commands::ci::checksums::HashReader;
use crate::commands::ci::checksums::Hashed;
use crate::commands::ruby::run::CaptureOutput;
use crate::commands::ruby::run::Invocation;
use crate::config::Config;
use crate::progress::WorkProgress;
use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::io::Write;
use std::ops::Not;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::time::Instant;
use std::vec;

mod checksums;

#[derive(Debug, clap_derive::Args)]
pub struct CleanInstallArgs {
    /// Path to Gemfile
    #[arg(long, env = "BUNDLE_GEMFILE")]
    gemfile: Option<Utf8PathBuf>,

    /// Maximum number of downloads that can be in flight at once.
    #[arg(long, hide = true, default_value = "10")]
    pub max_concurrent_requests: usize,

    /// Maximum number of gem installations that can be in flight at once.
    /// This reduces concurrently-open files on your filesystem,
    /// and concurrent disk operations.
    #[arg(long, hide = true, default_value = "20")]
    pub max_concurrent_installs: usize,

    /// Validate the checksums from the gem server and gem itself.
    #[arg(long, hide = true, default_value = "true")]
    pub validate_checksums: bool,

    /// Don't compile the extensions in native gems.
    #[arg(long, hide = true, default_value = "false")]
    pub skip_compile_extensions: bool,
}

#[derive(Debug)]
struct CiInnerArgs {
    pub skip_compile_extensions: bool,
    pub max_concurrent_requests: usize,
    pub max_concurrent_installs: usize,
    pub validate_checksums: bool,
    pub install_path: Utf8PathBuf,
    pub extensions_dir: Utf8PathBuf,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum UnpackError {
    #[error("No gemspec found for downloaded gem {0}")]
    MissingGemspec(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("File {filename} did not match {algo} checksum in gem {gem_name} archive")]
    ArchiveChecksumFail {
        filename: String,
        gem_name: String,
        algo: &'static str,
    },
    #[error("Checksum for {0} was not valid YAML")]
    InvalidChecksum(String),
    #[error("Gem {gem_name} archive did not include metadata.gz")]
    NoMetadata { gem_name: String },
    #[error("Gem archive did not include data.tar.gz")]
    NoDataTar,
    #[error("Invalid gem archive: {0}")]
    InvalidGemArchive(String),
    #[error("Could not parse YAML metadata inside gem package")]
    #[diagnostic(transparent)]
    YamlParsing(#[diagnostic_source] miette::Report),
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
    #[error("Needed to install Ruby but couldn't: {0}")]
    Install(#[from] crate::commands::ruby::install::Error),
    #[error("Cannot build unknown native extension {filename} from gem {gemname}")]
    UnknownExtension { filename: String, gemname: String },
    #[error("Error evaluating gemspec: {0}")]
    GemspecError(String),
    #[error("rv ci needs a Gemfile, but could not find it")]
    MissingImplicitGemfile,
    #[error("Gemfile \"{0}\" does not exist")]
    MissingGemfile(String),
    #[error("A {lockfile_name} file was not found in {lockfile_dir}")]
    MissingLockfile {
        lockfile_name: String,
        lockfile_dir: String,
    },
    #[error("Gem {gem} could not compile extensions")]
    CompileFailures { gem: String },
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
    #[error("File {filename} did not match {algo} locked checksum in gem {gem_name}")]
    LockfileChecksumFail {
        filename: String,
        gem_name: String,
        algo: &'static str,
    },
    #[error("Could not write binstub for {dep_name}/{exe_name}: {error}")]
    CouldNotWriteBinstub {
        dep_name: String,
        exe_name: String,
        error: io::Error,
    },
    #[error("Could not download a git dependency: {error}")]
    Git { error: String },
    #[error(
        "The gemfile path must be inside a directory with a parent, but it wasn't. Path was {0}"
    )]
    InvalidGemfilePath(String),
    #[error(transparent)]
    UnpackError(#[from] UnpackError),
}

type Result<T> = std::result::Result<T, Error>;
type UnpackResult<T> = std::result::Result<T, UnpackError>;

pub async fn ci(config: &Config, args: CleanInstallArgs) -> Result<()> {
    // We need some Ruby installed, because we need to run Ruby code when installing
    // gems. Ensure Ruby is installed here so we can use it later.
    let ruby_request = config.ruby_request();
    if config.matching_ruby(&ruby_request).is_none() {
        crate::ruby_install(config, None, Some(ruby_request.clone()), None).await?;
    }

    // Now that it's installed, we can use Ruby to query various directories
    // we'll need to know later.
    let extensions_dir = find_exts_dir(config, &ruby_request)?;
    let (lockfile_dir, lockfile_path, gemfile_name) = find_manifest_paths(&args.gemfile)?;
    let install_path = find_install_path(config, &lockfile_dir, &ruby_request, gemfile_name)?;
    let inner_args = CiInnerArgs {
        skip_compile_extensions: args.skip_compile_extensions,
        max_concurrent_requests: args.max_concurrent_requests,
        max_concurrent_installs: args.max_concurrent_installs,
        validate_checksums: args.validate_checksums,
        install_path,
        extensions_dir,
    };

    // Terminal progress indicator (OSC 9;4) for supported terminals
    let progress = WorkProgress::new();

    // Initial phase: parse lockfile, handle path gems and git repos
    let span = info_span!("Parsing lockfile");
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name}").unwrap());

    let lockfile_contents = {
        let _guard = span.enter();
        tokio::fs::read_to_string(&lockfile_path).await?
    };
    let lockfile = rv_lockfile::parse(&lockfile_contents)?;

    drop(span);

    ci_inner_work(config, &inner_args, &progress, lockfile)
        .await
        .map(|_| ())
}

pub struct InstallStats {
    pub executables_installed: usize,
}

pub async fn install_from_lockfile(
    config: &Config,
    lockfile: GemfileDotLock<'_>,
    install_path: Utf8PathBuf,
) -> Result<InstallStats> {
    // We need some Ruby installed, because we need to run Ruby code when installing
    // gems. Ensure Ruby is installed here so we can use it later.
    let ruby_request = config.ruby_request();
    if config.matching_ruby(&ruby_request).is_none() {
        crate::ruby_install(config, None, Some(ruby_request.clone()), None).await?;
    }

    let inner_args = CiInnerArgs {
        skip_compile_extensions: false,
        max_concurrent_requests: 10,
        max_concurrent_installs: 20,
        validate_checksums: true,
        install_path,
        extensions_dir: find_exts_dir(config, &ruby_request)?,
    };

    // Terminal progress indicator (OSC 9;4) for supported terminals
    let progress = WorkProgress::new();

    // Do the work.
    ci_inner_work(config, &inner_args, &progress, lockfile).await
}

async fn ci_inner_work(
    config: &Config,
    args: &CiInnerArgs,
    progress: &WorkProgress,
    lockfile: GemfileDotLock<'_>,
) -> Result<InstallStats> {
    let binstub_dir = args.install_path.join("bin");
    tokio::fs::create_dir_all(&binstub_dir).await?;

    // Phase 1: Downloads (0-40%)
    let gem_count = lockfile.gem_spec_count() as u64;
    progress.start_phase(gem_count, 40);

    let path_fetch_start = Instant::now();
    let path_specs = install_paths(config, &lockfile.path, args)?;
    let path_count = path_specs.len();
    let path_fetch_elapsed = path_fetch_start.elapsed();

    let git_fetch_start = Instant::now();
    let git_specs = install_git_repos(config, &lockfile.git, args)?;
    let git_count = git_specs.len();
    let git_fetch_elapsed = git_fetch_start.elapsed();

    let gem_fetch_start = Instant::now();
    let stats = DownloadStats::default();
    let downloaded = download_gems(lockfile.clone(), &config.cache, args, progress, &stats).await?;
    let downloaded_count = downloaded.len();
    let gem_fetch_elapsed = gem_fetch_start.elapsed();

    let fetch_elapsed = path_fetch_elapsed + git_fetch_elapsed + gem_fetch_elapsed;

    // Phase 2: Installs (40-80%)
    progress.start_phase(downloaded_count as u64, 40);

    let install_start = Instant::now();
    let specs = install_gems(config, downloaded, args, progress)?;
    let gem_count = specs.len();
    let executables_installed = specs.iter().map(|spec| spec.executables.len()).sum();
    let install_elapsed = install_start.elapsed();

    // Phase 3 (Compiles, 80-100%) - start_phase called inside compile_gems after filtering
    let compile_start = Instant::now();
    let compiled_count = compile_gems(config, specs, args, progress)?;
    let compile_elapsed = compile_start.elapsed();

    let total_elapsed = fetch_elapsed + install_elapsed + compile_elapsed;
    let total_gems = gem_count + git_count + path_count;

    let (cached_count, network_count) = stats.counts();

    println!("{} gems installed:", total_gems);
    println!(
        " - {} fetching {} gems from gem servers ({} cached, {} downloaded), {} from git repos, {} from local paths",
        format_duration(fetch_elapsed),
        gem_count,
        cached_count,
        network_count,
        git_count,
        path_count,
    );
    println!(
        " - {} unpacking {} gems from gem servers",
        format_duration(install_elapsed),
        gem_count,
    );
    if compiled_count > 0 {
        println!(
            " - {} compiling {} native extensions",
            format_duration(compile_elapsed),
            compiled_count,
        );
    }
    println!(" - {} total", format_duration(total_elapsed));

    Ok(InstallStats {
        executables_installed,
    })
}

fn install_paths<'i>(
    config: &Config,
    path_sources: &Vec<rv_lockfile::datatypes::PathSection<'i>>,
    args: &CiInnerArgs,
) -> Result<Vec<GemSpecification>> {
    use rayon::prelude::*;

    debug!("Installing path gems");
    let span = info_span!("Installing path gems");
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name}").unwrap());
    let _guard = span.enter();

    let pool = create_rayon_pool(args.max_concurrent_installs).unwrap();
    let path_specs = pool.install(|| {
        let path_source_specs = path_sources
            .iter()
            .par_bridge()
            .map(|path_source| install_path(path_source, config, args))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok::<_, Error>(path_source_specs)
    })?;
    Ok(path_specs)
}

fn install_path(
    path_section: &rv_lockfile::datatypes::PathSection,
    config: &Config,
    args: &CiInnerArgs,
) -> Result<Vec<GemSpecification>> {
    let cached_gemspecs_dir = config
        .cache
        .shard(rv_cache::CacheBucket::Gemspec, "gemspecs")
        .into_path_buf();
    fs_err::create_dir_all(&cached_gemspecs_dir)?;

    let path_key = rv_cache::cache_digest(path_section.remote);
    let path_dir = Utf8PathBuf::from(path_section.remote);

    let mut path_specs = Vec::new();
    let pattern = path_dir.join("**/*.gemspec").to_string();
    for path in glob(&pattern).expect("invalid glob pattern").flatten() {
        debug!("found gemspec at {:?}", path);
        // find the .gemspec file(s)
        let dep = path_section.specs.iter().find(|s| {
            path.to_string_lossy()
                .contains(&format!("{}.gemspec", s.gem_version.name))
        });

        if let Some(dep) = dep {
            let gemname = dep.gem_version;
            let cache_key = format!("{path_key}-{gemname}.gemspec");
            let cached_gemspec_path = cached_gemspecs_dir.join(&cache_key);

            let cached = std::fs::exists(&cached_gemspec_path).is_ok_and(|exists| exists) && {
                let gemspec_modified = std::fs::metadata(&path)?.modified()?;
                let cache_modified = std::fs::metadata(&cached_gemspec_path)?.modified()?;
                gemspec_modified < cache_modified
            };
            // parse the YAML gemspec to get the executable names
            let dep_gemspec = if cached {
                let yaml_contents = std::fs::read_to_string(&cached_gemspec_path)?;

                match rv_gem_specification_yaml::parse(&yaml_contents) {
                    Ok(parsed) => parsed,
                    Err(_) => cache_gemspec_path(config, &path_dir, path, cached_gemspec_path)?,
                }
            } else {
                cache_gemspec_path(config, &path_dir, path, cached_gemspec_path)?
            };

            path_specs.push(dep_gemspec.clone());

            // pass the executable names to generate binstubs
            let binstub_dir = args.install_path.join("bin");
            install_binstub(&dep_gemspec.name, &dep_gemspec.executables, &binstub_dir)?;
        }
    }

    Ok(path_specs)
}

fn install_git_repos<'i>(
    config: &Config,
    git_sources: &Vec<rv_lockfile::datatypes::GitSection<'i>>,
    args: &CiInnerArgs,
) -> Result<Vec<GemSpecification>> {
    let span = info_span!("Fetching git gems");
    span.pb_set_style(&ProgressStyle::with_template("{spinner:.green} {span_name}").unwrap());
    let _guard = span.enter();

    let repos = download_git_repos(git_sources, &config.cache, args)?;

    debug!("Installing git gems");
    let git_gems_dir = args.install_path.join("bundler/gems");

    use rayon::prelude::*;
    let pool = create_rayon_pool(args.max_concurrent_installs).unwrap();
    let git_specs = pool.install(|| {
        let git_source_specs = repos
            .iter()
            .par_bridge()
            .map(|repo| install_git_repo(repo, &git_gems_dir, config, args))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok::<_, Error>(git_source_specs)
    })?;
    Ok(git_specs)
}

fn install_git_repo(
    repo: &DownloadedGitRepo,
    git_gems_dir: &Utf8Path,
    config: &Config,
    args: &CiInnerArgs,
) -> Result<Vec<GemSpecification>> {
    debug!("Installing git repo {:?}", repo);
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

    if repo.submodules {
        let get_submodules = std::process::Command::new("git")
            .current_dir(&dest_dir)
            .args([
                "git",
                "submodule",
                "update",
                "--quiet",
                "--init",
                "--recursive",
            ])
            .spawn()?
            .wait()?;
        if !get_submodules.success() {
            return Err(Error::Git {
                error: format!("git submodule update had exit code {}", get_submodules),
            });
        }
    }

    debug!("Installed repo {}", &repo_name);

    let cached_gemspecs_dir = config
        .cache
        .shard(rv_cache::CacheBucket::Gemspec, "gemspecs")
        .into_path_buf();
    fs_err::create_dir_all(&cached_gemspecs_dir)?;

    let mut git_specs = Vec::new();
    let pattern = dest_dir.join("**/*.gemspec").to_string();
    for path in glob(&pattern).expect("invalid glob pattern").flatten() {
        debug!("found gemspec at {:?}", path);
        // find the .gemspec file(s)
        let gitsha = &repo.sha;
        let dep = repo.specs.iter().find(|s| {
            path.to_string_lossy()
                .contains(&format!("{}.gemspec", s.gem_version.name))
        });
        if let Some(dep) = dep {
            // check the cache for "gitsha-gemname.gemspec", if not:
            let gemname = dep.gem_version;
            let cache_key = format!("{gitsha}-{gemname}.gemspec");
            let cached_gemspec_path = cached_gemspecs_dir.join(&cache_key);
            let cached = std::fs::exists(&cached_gemspec_path).is_ok_and(|exists| exists);
            let dep_gemspec = if cached {
                let yaml_contents = std::fs::read_to_string(&cached_gemspec_path)?;

                match rv_gem_specification_yaml::parse(&yaml_contents) {
                    Ok(parsed) => parsed,
                    Err(_) => cache_gemspec_path(config, &repo.path, path, cached_gemspec_path)?,
                }
            } else {
                cache_gemspec_path(config, &repo.path, path, cached_gemspec_path)?
            };

            git_specs.push(dep_gemspec.clone());

            // pass the executable names to generate binstubs
            let binstub_dir = args.install_path.join("bin");
            install_binstub(&dep_gemspec.name, &dep_gemspec.executables, &binstub_dir)?;
        }
    }

    Ok(git_specs)
}

/// Note this is not async, it shells out to `git clone` so it will block.
fn download_git_repos<'i>(
    git_sources: &Vec<rv_lockfile::datatypes::GitSection<'i>>,
    cache: &rv_cache::Cache,
    args: &CiInnerArgs,
) -> Result<Vec<DownloadedGitRepo<'i>>> {
    debug!("Downloading git gems");

    // Download git repos to this dir.
    let git_clone_dir = cache
        .shard(rv_cache::CacheBucket::Git, "gits")
        .into_path_buf();
    fs_err::create_dir_all(&git_clone_dir)?;

    let pool = create_rayon_pool(args.max_concurrent_installs).unwrap();
    use rayon::prelude::*;
    let downloads = pool.install(|| {
        git_sources
            .par_iter()
            .map(|git_source| download_git_repo(&git_clone_dir, git_source))
            .collect::<Result<Vec<_>>>()
    })?;
    Ok(downloads)
}

/// Clones git repos from their remote, or looks them up in the cache if they're already downloaded.
fn download_git_repo<'i>(
    git_clone_dir: &Utf8Path,
    git_source: &GitSection<'i>,
) -> Result<DownloadedGitRepo<'i>> {
    // This will be the subdir within `git_clone_dir` that the git cloned repos are written to.
    let cache_key = rv_cache::cache_digest((git_source.remote, git_source.revision));
    let git_repo_dir = git_clone_dir.join(&cache_key);

    // Check if it's already in the cache.
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
        // It wasn't cached, so clone it.
        tracing::event!(tracing::Level::DEBUG, %git_clone_dir, %git_source.remote, %git_source.revision, "Cloning repo");
        let git_cloned = std::process::Command::new("git")
            .current_dir(git_clone_dir)
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
    Ok(DownloadedGitRepo {
        remote: git_source.remote.to_string(),
        specs: git_source.specs.clone(),
        sha: git_source.revision.to_string(),
        path: git_repo_dir,
        submodules: git_source.submodules,
    })
}

fn cache_gemspec_path(
    config: &Config,
    path_dir: &Utf8PathBuf,
    path: PathBuf,
    cached_path: Utf8PathBuf,
) -> Result<GemSpecification> {
    let gemspec_path = Utf8PathBuf::try_from(path).expect("gemspec path not valid UTF-8");
    // shell out to ruby -e 'puts Gem::Specification.load("name.gemspec").to_yaml' to get the YAML-format gemspec as a string
    let result = crate::commands::ruby::run::run_no_install(
        Invocation::ruby(vec![]),
        config,
        &config.ruby_request(),
        &[
            "-e",
            &format!(
                "puts Gem::Specification.load(\"{}\").to_yaml",
                gemspec_path.canonicalize_utf8()?,
            ),
        ],
        CaptureOutput::Both,
        Some(path_dir),
    )?;

    let error = String::from_utf8(result.stderr).unwrap();

    if !result.status.success() {
        return Err(Error::GemspecError(error));
    }

    let yaml_contents = String::from_utf8(result.stdout).unwrap();

    let dep_gemspec = rv_gem_specification_yaml::parse(&yaml_contents)
        .expect("Failed to parse the result of RubyGems YAML serialization");

    debug!("writing YAML gemspec to {}", &cached_path);
    fs_err::write(&cached_path, &yaml_contents)?;

    Ok(dep_gemspec)
}

fn find_manifest_paths(
    gemfile: &Option<Utf8PathBuf>,
) -> Result<(Utf8PathBuf, Utf8PathBuf, String)> {
    let gemfile_name = gemfile
        .clone()
        .map_or("Gemfile".to_string(), |g| g.to_string());
    let gemfile_path = Utf8PathBuf::from(gemfile_name.clone());

    if !gemfile_path.exists() {
        if gemfile.is_none() {
            return Err(Error::MissingImplicitGemfile);
        } else {
            return Err(Error::MissingGemfile(gemfile_name));
        }
    }

    let lockfile_dir = gemfile_path
        .canonicalize_utf8()?
        .parent()
        .ok_or(Error::InvalidGemfilePath(gemfile_name.clone()))?
        .to_string();

    let lockfile_path = Utf8PathBuf::from(format!("{}.lock", gemfile_path));

    if !lockfile_path.exists() {
        let lockfile_name = lockfile_path.file_name().unwrap().to_string();

        return Err(Error::MissingLockfile {
            lockfile_dir,
            lockfile_name,
        });
    }

    debug!("found lockfile_path {}", lockfile_path);
    Ok((lockfile_dir.into(), lockfile_path, gemfile_name))
}

/// Which path should `ci` install gems under?
/// Uses Bundler's `bundle_path`.
fn find_install_path(
    config: &Config,
    lockfile_dir: &Utf8Path,
    version: &RubyRequest,
    gemfile: String,
) -> Result<Utf8PathBuf> {
    let args = ["-rbundler", "-e", "puts Bundler.bundle_path"];
    let bundle_path = match crate::commands::ruby::run::run_no_install(
        Invocation::ruby(vec![("BUNDLE_GEMFILE", gemfile)]),
        config,
        version,
        args.as_slice(),
        CaptureOutput::Both,
        Some(lockfile_dir),
    ) {
        Ok(output) => output.stdout,
        Err(_) => Vec::from(".rv"),
    };

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

fn install_gems<'i>(
    config: &Config,
    downloaded: Vec<DownloadedRubygems<'i>>,
    args: &CiInnerArgs,
    progress: &WorkProgress,
) -> Result<Vec<GemSpecification>> {
    use rayon::prelude::*;

    debug!("Installing gem packages");
    let span = info_span!("Installing gem packages");
    span.pb_set_style(
        &ProgressStyle::with_template("{spinner:.green} {span_name} {pos}/{len}").unwrap(),
    );
    span.pb_set_length(downloaded.len() as u64);
    let _guard = span.enter();

    let binstub_dir = args.install_path.join("bin");
    let pool = create_rayon_pool(args.max_concurrent_installs).unwrap();
    let specs = pool.install(|| {
        downloaded
            .into_iter()
            .par_bridge()
            .map(|download| {
                let result = install_single_gem(config, download, args, &binstub_dir);
                span.pb_inc(1);
                progress.complete_one();
                result
            })
            .collect::<Result<Vec<GemSpecification>>>()
    })?;

    Ok(specs)
}

fn install_single_gem<'i>(
    config: &Config,
    download: DownloadedRubygems<'i>,
    args: &CiInnerArgs,
    binstub_dir: &Utf8Path,
) -> Result<GemSpecification> {
    let gv = download.spec.gem_version;
    // Actually unpack the tarball here.
    let dep_gemspec_res = download.unpack_tarball(config, &args.install_path, args)?;
    debug!("Unpacked tarball {gv}");
    let dep_gemspec = dep_gemspec_res.ok_or(UnpackError::MissingGemspec(gv.to_string()))?;
    debug!("Installing binstubs for {gv}");
    install_binstub(&dep_gemspec.name, &dep_gemspec.executables, binstub_dir)?;
    debug!("Installed {gv}");
    Ok(dep_gemspec)
}

fn compile_gems(
    config: &Config,
    specs: Vec<GemSpecification>,
    args: &CiInnerArgs,
    progress: &WorkProgress,
) -> Result<usize> {
    if args.skip_compile_extensions {
        return Ok(0);
    }

    use dep_graph::{DepGraph, Node};
    use rayon::prelude::*;

    let mut nodes: HashMap<String, &GemSpecification> = HashMap::new();

    let deps: Vec<_> = specs
        .iter()
        .map(|spec| {
            let name = spec.name.clone();
            let mut node = Node::new(name.clone());

            if !spec.extensions.is_empty() {
                nodes.insert(name, spec);
            }

            for dep in &spec.dependencies {
                if dep.is_runtime() {
                    node.add_dep(dep.name.clone());
                }
            }

            node
        })
        .collect();

    if deps.is_empty() {
        return Ok(0);
    }

    let deps_count = nodes.len();

    // Phase 3: Compiles (80-100%)
    progress.start_phase(deps_count as u64, 20);

    debug!("Compiling gem packages");
    let span = info_span!("Compiling native extensions");
    span.pb_set_style(
        &ProgressStyle::with_template("{spinner:.green} {span_name} ({pos}/{len}) - {msg}")
            .unwrap(),
    );
    span.pb_set_length(deps_count as u64);
    let _guard = span.enter();

    let graph = DepGraph::new(deps.as_slice());
    graph.into_par_iter().try_for_each(|node| {
        if let Some(spec) = nodes.get(&*node) {
            span.pb_set_message(&spec.name);
            let compiled_ok = compile_gem(config, args, spec)?;
            span.pb_inc(1);
            progress.complete_one();
            if !compiled_ok {
                return Err(Error::CompileFailures {
                    gem: spec.full_name(),
                });
            }
        }
        Ok(())
    })?;

    Ok(deps_count)
}

fn install_binstub(dep_name: &str, executables: &[String], binstub_dir: &Utf8Path) -> Result<()> {
    for exe_name in executables {
        debug!("Creating binstub {dep_name}-{exe_name}");
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

/// Tracks how many gems were served from cache vs downloaded from the network.
#[derive(Default)]
struct DownloadStats {
    cached: AtomicU64,
    downloaded: AtomicU64,
}

impl DownloadStats {
    fn cached_one(&self) {
        self.cached.fetch_add(1, Ordering::Relaxed);
    }

    fn downloaded_one(&self) {
        self.downloaded.fetch_add(1, Ordering::Relaxed);
    }

    fn counts(&self) -> (u64, u64) {
        (
            self.cached.load(Ordering::Relaxed),
            self.downloaded.load(Ordering::Relaxed),
        )
    }
}

/// Downloads all Rubygem server gems from a Gemfile.lock
async fn download_gems<'i>(
    lockfile: GemfileDotLock<'i>,
    cache: &rv_cache::Cache,
    args: &CiInnerArgs,
    progress: &WorkProgress,
    stats: &DownloadStats,
) -> Result<Vec<DownloadedRubygems<'i>>> {
    debug!("Downloading gem packages");
    let span = info_span!("Downloading gem packages");
    span.pb_set_style(
        &ProgressStyle::with_template("{spinner:.green} {span_name} {pos}/{len} - {msg}").unwrap(),
    );
    span.pb_set_length(lockfile.gem_spec_count() as u64);
    span.pb_set_message("0 cached, 0 downloaded");
    let _guard = span.enter();

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
            let checksums = &checksums;
            let span = &span;
            async move {
                download_gem_source(
                    gem_source,
                    checksums,
                    cache,
                    args.max_concurrent_requests,
                    progress,
                    stats,
                    span,
                )
                .await
            }
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
    submodules: bool,
}

impl<'i> DownloadedRubygems<'i> {
    fn unpack_tarball(
        self,
        config: &Config,
        bundle_path: &Utf8PathBuf,
        args: &CiInnerArgs,
    ) -> Result<Option<GemSpecification>> {
        match self.unpack_tarball_inner(config, bundle_path, args) {
            Err(error) => {
                // Print out nice Miette reports
                if matches!(error, UnpackError::YamlParsing(_)) {
                    println!("{error:?}");
                };

                std::fs::remove_dir_all(bundle_path).unwrap();

                Err(Error::UnpackError(error))
            }
            Ok(other) => Ok(other),
        }
    }

    fn unpack_tarball_inner(
        self,
        config: &Config,
        bundle_path: &Utf8PathBuf,
        args: &CiInnerArgs,
    ) -> UnpackResult<Option<GemSpecification>> {
        // Unpack the tarball into DIR/gems/
        // It should contain a metadata zip, and a data zip
        // (and optionally, a checksum zip).
        let GemVersion { name, version } = self.spec.gem_version;
        let full_name = format!("{name}-{version}");
        debug!("Unpacking {full_name}");

        // Then unpack the tarball into it.
        let contents = &self.contents[..];

        // Now that we've handled checksums (perhaps), we can iterate through the archive
        // and unpack the entries we care about. Specifically the metadata and the data itself.
        // If we found checksums, validate them.
        let mut found_gemspec = None;
        let mut checksums: Option<ArchiveChecksums> = None;
        let mut archive = tar::Archive::new(contents);
        let mut metadata_hashed = None;
        let mut data_tar_unpacked = None;
        for e in archive.entries()? {
            let entry = e?;
            let entry_path = entry.path()?;
            match entry_path.display().to_string().as_str() {
                "checksums.yaml.gz" => {
                    let mut contents = GzDecoder::new(entry);
                    let mut str_contents = String::new();
                    let _ = contents.read_to_string(&mut str_contents)?;
                    let cs = ArchiveChecksums::new(&str_contents).ok_or(
                        UnpackError::InvalidChecksum(self.spec.gem_version.to_string()),
                    )?;

                    // Should not happen in practice, because we break after finding the checksums.
                    // But may as well be defensive here.
                    if checksums.replace(cs).is_some() {
                        return Err(UnpackError::InvalidGemArchive(
                            "two checksums.yaml.gz files found in the gem archive".to_owned(),
                        ));
                    }
                }
                "metadata.gz" => {
                    // Unpack the metadata, which stores the gem specs.
                    if found_gemspec.is_some() {
                        return Err(UnpackError::InvalidGemArchive(
                            "two metadata.gz found".to_owned(),
                        ));
                    }
                    let UnpackedMetadata { hashed, gemspec } =
                        unpack_metadata(config, bundle_path, &full_name, HashReader::new(entry))?;
                    found_gemspec = Some(gemspec);
                    metadata_hashed = Some(hashed);
                }
                "data.tar.gz" => {
                    // Unpack the data archive, which stores all the gems.
                    if data_tar_unpacked.is_some() {
                        return Err(UnpackError::InvalidGemArchive(
                            "two data.tar.gz found".to_owned(),
                        ));
                    }
                    let unpacked =
                        unpack_data_tar(bundle_path, &full_name, HashReader::new(entry))?;
                    data_tar_unpacked = Some(unpacked);
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

        let Some(data_tar_unpacked) = data_tar_unpacked else {
            return Err(UnpackError::NoDataTar);
        };
        if found_gemspec.is_none() {
            return Err(UnpackError::NoMetadata {
                gem_name: full_name,
            });
        };
        if args.validate_checksums
            && let Some(ref checksums) = checksums
        {
            if let Some(hashed) = metadata_hashed {
                checksums.validate_metadata(full_name.clone(), hashed)?
            }

            checksums.validate_data_tar(full_name, &data_tar_unpacked.hashed)?
        }

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
    fs_err::write(&binstub_path, binstub_contents)?;
    fs_err::set_permissions(binstub_path, PermissionsExt::from_mode(0o755))
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

fn find_exts_dir(config: &Config, version: &RubyRequest) -> Result<Utf8PathBuf> {
    debug!("Finding extensions dir");
    let exts_dir = crate::commands::ruby::run::run_no_install(
        Invocation::ruby(vec![]),
        config,
        version,
        &[
            "-e",
            "puts File.join(Gem::Platform.local.to_s, Gem.extension_api_version)",
        ],
        CaptureOutput::Both,
        None,
    )?
    .stdout;

    let extensions_dir = String::from_utf8(exts_dir)
        .map(|s| Utf8PathBuf::from(s.trim()))
        .map_err(|_| Error::BadBundlePath)?;
    debug!("Found extensions dir: {extensions_dir}");
    Ok(extensions_dir)
}

static EXTCONF_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)extconf").unwrap());
static RAKE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)rakefile|mkrf_conf").unwrap());

fn compile_gem(config: &Config, args: &CiInnerArgs, spec: &GemSpecification) -> Result<bool> {
    let mut compile_results = Vec::with_capacity(spec.extensions.len());

    let gem_home = &args.install_path;
    let gem_path = gem_home.join("gems").join(spec.full_name());
    let lib_dest = gem_path.join("lib");
    let ext_dest = args
        .install_path
        .join("extensions")
        .join(&args.extensions_dir)
        .join(spec.full_name());
    let mut ran_rake = false;

    if std::fs::exists(ext_dest.join("gem.build_complete"))? {
        debug!("native extensions for {} already built", spec.full_name());
        return Ok(true);
    }
    debug!("compiling native extensions for {}", spec.full_name());

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
                    build_rakefile(config, extension, gem_home, &gem_path, &ext_dest, &lib_dest)
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

    // 1. Run mkrf if needed to create the Rakefile
    if ext_file.to_lowercase().contains("mkrf_conf") {
        output = crate::commands::ruby::run::run_no_install(
            Invocation::ruby(vec![]),
            config,
            &config.ruby_request(),
            &[ext_file],
            CaptureOutput::Both,
            Some(&ext_dir),
        )?;
        outputs.push(output);
    }

    // 2. Run Rake with the args
    let tmp_dir = camino_tempfile::tempdir_in(gem_path)?;
    let sitearchdir = format!("RUBYARCHDIR={}", tmp_dir.path());
    let sitelibdir = format!("RUBYLIBDIR={}", tmp_dir.path());
    let args = vec![sitearchdir, sitelibdir];

    let rake = Invocation::tool("rake", vec![("GEM_HOME", gem_home.to_string())]);

    output = crate::commands::ruby::run::run_no_install(
        rake,
        config,
        &config.ruby_request(),
        &args,
        CaptureOutput::Both,
        Some(&ext_dir),
    )?;
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
        Invocation::ruby(vec![("GEM_HOME", gem_home.to_string())]),
        config,
        &config.ruby_request(),
        &[ext_file],
        CaptureOutput::Both,
        Some(&ext_dir),
    )?;
    outputs.push(output);

    // 2. Save the mkmf.log file if it exists
    let mkmf_log = ext_dir.join("mkmf.log");
    if mkmf_log.exists() {
        fs_err::create_dir_all(ext_dest)?;
        fs_err::rename(mkmf_log, ext_dest.join("mkmf.log"))?;
    }

    // 3. Run make clean / make / make install / make clean
    //
    // Use run_no_install with Invocation::tool to ensure Ruby is in PATH.
    // This is needed for gems that use rb-sys (Rust-based extensions) because
    // their Cargo build scripts call `ruby` to query RbConfig.
    let tmp_dir = camino_tempfile::tempdir_in(gem_path)?;
    let sitearchdir = format!("sitearchdir={}", tmp_dir.path());
    let sitelibdir = format!("sitelibdir={}", tmp_dir.path());
    let destdir = "DESTDIR=''".to_string();
    let base_args = vec![destdir.as_str(), sitearchdir.as_str(), sitelibdir.as_str()];
    let make_env = vec![("GEM_HOME", gem_home.to_string())];

    // make clean (ignore failures)
    let _ = crate::commands::ruby::run::run_no_install(
        Invocation::tool("make", make_env.clone()),
        config,
        &config.ruby_request(),
        &[&["clean"], base_args.as_slice()].concat(),
        CaptureOutput::Both,
        Some(&ext_dir),
    );

    // make
    output = crate::commands::ruby::run::run_no_install(
        Invocation::tool("make", make_env.clone()),
        config,
        &config.ruby_request(),
        &base_args,
        CaptureOutput::Both,
        Some(&ext_dir),
    )?;
    let success = output.status.success();
    outputs.push(output);
    if !success {
        return Ok(outputs);
    }

    // make install
    output = crate::commands::ruby::run::run_no_install(
        Invocation::tool("make", make_env.clone()),
        config,
        &config.ruby_request(),
        &[&["install"], base_args.as_slice()].concat(),
        CaptureOutput::Both,
        Some(&ext_dir),
    )?;
    outputs.push(output);

    // make clean (ignore failures)
    let _ = crate::commands::ruby::run::run_no_install(
        Invocation::tool("make", make_env),
        config,
        &config.ruby_request(),
        &[&["clean"], base_args.as_slice()].concat(),
        CaptureOutput::Both,
        Some(&ext_dir),
    );

    // 4. Copy the resulting files to ext and lib dirs
    copy_dir(&tmp_dir, lib_dest)?;
    copy_dir(&tmp_dir, ext_dest)?;

    // 5. Mark the gem as built
    fs_err::write(ext_dest.join("gem.build_complete"), "")?;

    Ok(outputs)
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
) -> UnpackResult<UnpackedData>
where
    R: std::io::Read,
{
    // First, create the data's destination.
    let data_dir: PathBuf = bundle_path.join("gems").join(nameversion).into();
    fs_err::create_dir_all(&data_dir)?;
    // Unpack it:
    let mut gem_data_archive = tar::Archive::new(GzDecoder::new(data_tar_gz));
    gem_data_archive.unpack(data_dir)?;
    // Get the HashReader back, so we can tell what the hash is for the contents of this tar.
    let h = gem_data_archive.into_inner().into_inner();
    let hashed = h.finalize();
    Ok(UnpackedData { hashed })
}

struct UnpackedMetadata {
    hashed: Hashed,
    gemspec: GemSpecification,
}

/// Given the metadata.gz from a gem, write it to the filesystem under
/// BUNDLEPATH/specifications/name-version.gemspec
fn unpack_metadata<R>(
    _config: &Config,
    bundle_path: &Utf8Path,
    nameversion: &str,
    metadata_gz: HashReader<R>,
) -> UnpackResult<UnpackedMetadata>
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
    let parsed =
        rv_gem_specification_yaml::parse(&yaml_contents).map_err(UnpackError::YamlParsing)?;
    let ruby_contents = rv_gem_specification_yaml::to_ruby(parsed.clone());
    std::io::copy(&mut ruby_contents.as_bytes(), &mut dst)?;

    let h = unzipper.into_inner();
    Ok(UnpackedMetadata {
        hashed: h.finalize(),
        gemspec: parsed,
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
    progress: &WorkProgress,
    stats: &DownloadStats,
    span: &tracing::Span,
) -> Result<Vec<DownloadedRubygems<'i>>> {
    // TODO: If the gem server needs user credentials, accept them and add them to this client.
    let client = rv_http_client()?;

    // Download them all, concurrently.
    //
    // Filter to gems matching local platform, preferring platform-specific gems
    // over generic "ruby" platform gems. This ensures we use prebuilt binaries
    // (like libv8-node-24.1.0.0-x86_64-linux.gem) instead of compiling from
    // source (libv8-node-24.1.0.0.gem).
    let gems_to_download = prefer_platform_specific_gems(gem_source.specs);
    let spec_stream = futures_util::stream::iter(gems_to_download);
    let downloaded_gems: Vec<_> = spec_stream
        .map(|spec| {
            let client = &client;
            async move {
                let result = download_gem(
                    gem_source.remote,
                    spec,
                    client,
                    cache,
                    checksums,
                    stats,
                    span,
                )
                .await;
                span.pb_inc(1);
                progress.complete_one();
                result
            }
        })
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
    stats: &DownloadStats,
    span: &tracing::Span,
) -> Result<DownloadedRubygems<'i>> {
    let url = url_for_spec(remote, &spec)?;
    let cache_key = rv_cache::cache_digest(url.as_ref());
    let cache_path = cache
        .shard(rv_cache::CacheBucket::Gem, "gems")
        .into_path_buf()
        .join(format!("{cache_key}.gem"));

    let contents = if cache_path.exists() {
        debug!("Reusing gem from {url} in cache");
        stats.cached_one();
        let data = tokio::fs::read(&cache_path).await?;
        Bytes::from(data)
    } else {
        debug!("Downloading gem from {url}");
        stats.downloaded_one();
        client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?
    };

    // Update the progress bar message with current stats
    let (cached, downloaded) = stats.counts();
    span.pb_set_message(&format!("{cached} cached, {downloaded} downloaded"));

    // Validate the checksums.
    if let Some(checksum) = checksums.get(&spec.gem_version) {
        match checksum.algorithm {
            KnownChecksumAlgos::Sha256 => {
                let actual = sha2::Sha256::digest(&contents);
                if actual[..] != checksum.value {
                    return Err(Error::LockfileChecksumFail {
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

/// Filter gem specs to those matching local platform, preferring platform-specific
/// gems over generic "ruby" platform gems.
///
/// When a lockfile contains both a generic ruby gem (e.g., `libv8-node-24.1.0.0`)
/// and platform-specific variants (e.g., `libv8-node-24.1.0.0-x86_64-linux`),
/// this function ensures we download the platform-specific prebuilt binary
/// instead of the generic gem that would require compiling from source.
fn prefer_platform_specific_gems<'i>(specs: Vec<Spec<'i>>) -> Vec<Spec<'i>> {
    // Group specs by gem name, keeping only those matching local platform
    let mut by_name: HashMap<&str, Vec<Spec<'i>>> = HashMap::new();
    for spec in specs {
        let Ok(vp) = VersionPlatform::from_str(spec.gem_version.version) else {
            continue;
        };
        if vp.platform.is_local() {
            by_name.entry(spec.gem_version.name).or_default().push(spec);
        }
    }

    // For each gem, pick the best platform variant using VersionPlatform ordering
    // (Platform::Ruby < Platform::Specific, so max_by prefers specific)
    by_name
        .into_values()
        .filter_map(|candidates| {
            candidates.into_iter().max_by(|a, b| {
                let vp_a = VersionPlatform::from_str(a.gem_version.version).ok();
                let vp_b = VersionPlatform::from_str(b.gem_version.version).ok();
                vp_a.cmp(&vp_b)
            })
        })
        .collect()
}

/// Format a duration in a human-readable way (e.g., "16s" or "1m16s").
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs >= 60 {
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        format!("{}m{}s", mins, remaining_secs)
    } else {
        format!("{:.1}s", duration.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_gem_types::Platform;

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
    fn test_platform_current() {
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let expected = ("darwin", "x86_64");
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let expected = ("darwin", "arm64");
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let expected = ("linux", "x86_64");
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let expected = ("linux", "aarch64");
        let actual = Platform::local();
        let Platform::Specific {
            cpu: actual_cpu,
            os: actual_os,
            version: _,
        } = actual
        else {
            panic!("Platform should be specific");
        };
        assert_eq!(actual_cpu.unwrap(), expected.1);
        assert_eq!(actual_os, expected.0);
    }

    #[test]
    fn binary_checksums() {
        // Taken from https://rubygems.org/gems/options/versions/2.3.2
        // Apparently this is an old checksum format?
        let test_file = r#"---
!binary "U0hBMQ==":
  metadata.gz: !binary |-
    Y2IyYTYzNDkyOGEwM2E1ODQyNzY2MTFjNGIzYmYzNTU3MDc2NDRkNw==
  data.tar.gz: !binary |-
    NzkxMmRmMmViNjRiMWIyZmU1MWJhOWYyNjExZjIxMTk4OTc0YTY0YQ==
!binary "U0hBNTEy":
  metadata.gz: !binary |-
    ZGNlNjllYmRmYjZlNGI1ZjVmYjU4YWExNDhkOGRkMzA3YWU0MmE5YWI3MmJj
    NTE3MTJmZTIzNDMzNjUzNDRhODEyMmU0ODRlM2NkNmY3MTQyZWEzMDBjZmUw
    N2Y3ODAzNmQ0MzY0NWFkNzc1ZGViODhmMmI3ZmU4Y2M3ZmEwMDg=
  data.tar.gz: !binary |-
    MTk4ZGE5YjhlYTkwMTdmZmJjMDI0ZDA0NWIzODhlNDc0NWQzMzQ4NTNkYTI0
    ZjNkNjZmZTc5MWE3NjI2YmVlMDlhYjE4OTE0YTRhYmQxMTM0MWUyMjcyYTgx
    ZTU0N2NmMDRlYzNkY2NjOTBkYmI0MmFjM2RiMzgzM2FjM2U2N2Q=
"#;

        let checksums = ArchiveChecksums::new(test_file).unwrap();
        let sha512 = checksums
            .sha512
            .expect("this file does have a sha512 checksum");
        assert_eq!(
            "dce69ebdfb6e4b5f5fb58aa148d8dd307ae42a9ab72bc51712fe2343365344a8122e484e3cd6f7142ea300cfe07f78036d43645ad775deb88f2b7fe8cc7fa008",
            &hex::encode(sha512.metadata_gz)
        );
        assert_eq!(
            "198da9b8ea9017ffbc024d045b388e4745d334853da24f3d66fe791a7626bee09ab18914a4abd11341e2272a81e547cf04ec3dccc90dbb42ac3db3833ac3e67d",
            &hex::encode(sha512.data_tar_gz)
        );
    }

    #[test]
    fn binary_checksums_formatador() {
        // Taken from formatador 0.2.5
        let test_file = r#"---
!binary "U0hBMQ==":
  metadata.gz: !binary |-
    ZmFhYzcyNjZlYTkzM2RmZTBhYmY1OWJmN2ViOWUyMzYyNmIyYzdiMw==
  data.tar.gz: !binary |-
    MGRkYmM5ZGM0OTliYjI4NjEwY2M3NDZmYWI2NWYwOTZjM2ZkOWExMw==
SHA512:
  metadata.gz: !binary |-
    Y2VhYjM0NzZkYmExYmQ5OTMwZjI5ZmZhYTllNTBmNzdlMDEzNDk4NWVmZTVj
    MmU0ZjEwZTVhOTdlZWU3ZGYyOTE5YTc3YTA4Y2MxNDM3MDQyYmY0ZjMzNmY3
    YTIxNDY1MDMwM2ZiOGI1MzQ3ODJiYWRlZmNiMzIyOWFmNzBiZDY=
  data.tar.gz: !binary |-
    YzcyNzFhYTk2ODI2YjUzZTlhZTAxNWI0MTYzZmJlNGY2MjU2Y2M5MTAxY2Fl
    OTViZGRlMWVlODgwMWNmOWExNTZlYWY5YzFhNjc4Yzg3Y2IxZDQ5YjJjN2Iw
    OWUyYjI5ZjAyNDg3Mzk0YmVlMDA5MmU0ZDU3ZmMxOTUyYzRjNjI="#;
        let checksums = ArchiveChecksums::new(test_file).expect("should have found checksums");
        let sha512 = checksums
            .sha512
            .expect("this file does have a sha512 checksum");
        assert_eq!(
            "ceab3476dba1bd9930f29ffaa9e50f77e0134985efe5c2e4f10e5a97eee7df2919a77a08cc1437042bf4f336f7a214650303fb8b534782badefcb3229af70bd6",
            &hex::encode(sha512.metadata_gz)
        );
        assert_eq!(
            "c7271aa96826b53e9ae015b4163fbe4f6256cc9101cae95bdde1ee8801cf9a156eaf9c1a678c87cb1d49b2c7b09e2b29f02487394bee0092e4d57fc1952c4c62",
            &hex::encode(sha512.data_tar_gz)
        );
    }

    #[test]
    fn test_prefer_platform_specific_gems() {
        // Use the real Discourse lockfile fixture which has libv8-node with
        // multiple platform variants (ruby, x86_64-linux, aarch64-linux, etc.)
        let input = include_str!("../../../rv-lockfile/tests/inputs/Gemfile.discourse.lock");
        let lockfile = rv_lockfile::parse(input).expect("should parse discourse lockfile");

        // Get all specs from the gem sources
        let all_specs: Vec<_> = lockfile
            .gem
            .into_iter()
            .flat_map(|section| section.specs)
            .collect();

        // Count how many libv8-node variants exist before filtering
        let libv8_before: Vec<_> = all_specs
            .iter()
            .filter(|s| s.gem_version.name == "libv8-node")
            .collect();
        assert!(
            libv8_before.len() > 1,
            "fixture should have multiple libv8-node variants, found {}",
            libv8_before.len()
        );

        // Apply the filter
        let result = super::prefer_platform_specific_gems(all_specs);

        // Should only have ONE libv8-node after filtering
        let libv8_after: Vec<_> = result
            .iter()
            .filter(|s| s.gem_version.name == "libv8-node")
            .collect();
        assert_eq!(
            libv8_after.len(),
            1,
            "should have exactly one libv8-node after filtering, found {}",
            libv8_after.len()
        );

        // Verify the correct platform was chosen for the current machine
        let libv8 = libv8_after[0];

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let expected_version = "24.1.0.0-arm64-darwin";
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let expected_version = "24.1.0.0-x86_64-darwin";
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let expected_version = "24.1.0.0-aarch64-linux";
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let expected_version = "24.1.0.0-x86_64-linux";

        assert_eq!(
            libv8.gem_version.version, expected_version,
            "should select platform-specific version for current platform"
        );
    }
}
