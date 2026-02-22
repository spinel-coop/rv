use std::{
    env::{self, JoinPathsError, join_paths, split_paths},
    path::PathBuf,
    str::FromStr,
};

use camino::{FromPathBufError, Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use tracing::{debug, instrument};

use rv_ruby::{
    Ruby,
    request::{RequestError, RubyRequest, Source},
    version::{ParseVersionError, ReleasedRubyVersion, RubyVersion},
};

use crate::GlobalArgs;

pub mod github;
mod ruby_cache;
mod ruby_fetcher;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    NonUtf8Path(#[from] FromPathBufError),
    #[error("Ruby cache miss or invalid cache for {}", ruby_path)]
    RubyCacheMiss { ruby_path: Utf8PathBuf },
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    RequestError(#[from] RequestError),
    #[error(transparent)]
    EnvError(#[from] env::VarError),
    #[error(transparent)]
    JoinPathsError(#[from] JoinPathsError),
    #[error("Tried to parse ruby version from Gemfile.lock, but that file was invalid: {0}")]
    CouldNotParseGemfileLock(#[from] rv_lockfile::ParseErrors),
    #[error("Could not parse ruby version from Gemfile.lock: {0}")]
    CouldNotParseGemfileLockVersion(ParseVersionError),
    #[error("no matching ruby version found")]
    NoMatchingRuby,
}

type Result<T> = miette::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Config {
    pub ruby_dirs: IndexSet<Utf8PathBuf>,
    pub cache: rv_cache::Cache,
    pub current_exe: Utf8PathBuf,
    pub requested_ruby: RequestedRuby,
}

#[derive(Debug, Clone)]
pub enum RequestedRuby {
    Explicit(RubyRequest),
    Project((RubyRequest, Source)),
    User((RubyRequest, Source)),
    Global,
}

impl Config {
    pub(crate) fn new(global_args: &GlobalArgs, request: Option<RubyRequest>) -> Result<Self> {
        let root = Utf8PathBuf::from(env::var("RV_ROOT_DIR").unwrap_or("/".to_owned()));

        let ruby_dirs = if global_args.ruby_dir.is_empty() {
            default_ruby_dirs(&root)
        } else {
            global_args
                .ruby_dir
                .iter()
                .map(|path: &Utf8PathBuf| Ok(root.join(rv_dirs::canonicalize_utf8(path)?)))
                .collect::<Result<Vec<_>>>()?
        };
        let ruby_dirs: IndexSet<Utf8PathBuf> = ruby_dirs.into_iter().collect();
        let cache = global_args.cache_args.to_cache()?;
        let current_exe = if let Some(exe) = global_args.current_exe.clone() {
            exe
        } else {
            std::env::current_exe()?.to_str().unwrap().into()
        };

        let requested_ruby = match request {
            Some(req) => {
                debug!("Explicit ruby request for {} received", req);
                RequestedRuby::Explicit(req)
            }
            None => {
                let home_dir = rv_dirs::home_dir();
                let current_dir: Utf8PathBuf = std::env::current_dir()?.try_into()?;

                let project_root = current_dir
                    .ancestors()
                    .take_while(|d| Some(*d) != root.parent())
                    .find(|d| d.join("Gemfile.lock").is_file())
                    .map(|p| p.to_path_buf())
                    .unwrap_or(current_dir.clone());

                debug!("Found project directory in {}", project_root);

                if let Some(req) = find_directory_ruby(&project_root)? {
                    debug!("Found project ruby request for {} in {:?}", req.0, req.1);
                    RequestedRuby::Project(req)
                } else if let Some(req) = find_directory_ruby(&home_dir)? {
                    debug!("Found user ruby request for {} in {:?}", req.0, req.1);
                    RequestedRuby::User(req)
                } else {
                    RequestedRuby::Global
                }
            }
        };

        Ok(Self {
            ruby_dirs,
            cache,
            current_exe,
            requested_ruby,
        })
    }

    #[instrument(skip_all, level = "trace")]
    pub fn rubies(&self) -> Vec<Ruby> {
        self.discover_installed_rubies()
    }

    pub async fn remote_rubies(&self) -> Vec<Ruby> {
        self.discover_remote_rubies().await
    }

    pub async fn find_matching_remote_ruby(&self) -> Result<RubyVersion> {
        let requested_range = self.ruby_request();

        if let Ok(version) = RubyVersion::try_from(requested_range.clone()) {
            debug!(
                "Skipping the rv-ruby releases fetch because the user has given a specific ruby version {version}"
            );
            Ok(version)
        } else {
            debug!("Fetching available rubies, because user gave an underspecified Ruby range");
            let remote_rubies = self.remote_rubies().await;

            let matched_ruby = requested_range
                .find_match_in(&remote_rubies)
                .ok_or(Error::NoMatchingRuby)?;

            Ok(RubyVersion::Released(matched_ruby.version))
        }
    }

    pub fn current_ruby(&self) -> Option<Ruby> {
        let request = &self.ruby_request();

        self.discover_rubies_matching(|dir_name| {
            let version_res = RubyVersion::from_str(dir_name);
            version_res.is_ok_and(|v| v.satisfies(request))
        })
        .last()
        .cloned()
    }

    pub fn ruby_request(&self) -> RubyRequest {
        match &self.requested_ruby {
            RequestedRuby::Explicit(request) => request.clone(),
            RequestedRuby::Project((request, _)) => request.clone(),
            RequestedRuby::User((request, _)) => request.clone(),
            RequestedRuby::Global => RubyRequest::default(),
        }
    }
}

fn xdg_data_path() -> Utf8PathBuf {
    rv_dirs::user_state_dir("/".into()).join("rubies")
}

fn legacy_default_data_path() -> Utf8PathBuf {
    rv_dirs::home_dir().join(".data/rv/.rubies")
}

fn legacy_default_path() -> Utf8PathBuf {
    rv_dirs::home_dir().join(".rubies")
}

/// Default Ruby installation directories
pub fn default_ruby_dirs(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let paths: [(_, _); 6] = [
        (true, xdg_data_path()),
        (false, legacy_default_data_path()),
        (false, legacy_default_path()),
        (false, "/opt/rubies".into()),
        (false, "/usr/local/rubies".into()),
        (false, "/opt/homebrew/Cellar/ruby".into()),
    ];

    paths
        .iter()
        .filter_map(|(always_include, path)| {
            let join = root.join(path.strip_prefix("/").unwrap_or(path));
            rv_dirs::canonicalize_utf8(&join)
                .ok()
                .or(always_include.then_some(path.into()))
        })
        .collect()
}

fn find_directory_ruby(dir: &Utf8PathBuf) -> Result<Option<(RubyRequest, Source)>> {
    let ruby_version = dir.join(".ruby-version");
    if ruby_version.exists() {
        let ruby_version_string = std::fs::read_to_string(&ruby_version)?;
        return Ok(Some((
            ruby_version_string.parse()?,
            Source::DotRubyVersion(ruby_version),
        )));
    }

    let tool_versions = dir.join(".tool-versions");
    if tool_versions.exists() {
        let tool_versions_string = std::fs::read_to_string(&tool_versions)?;
        let tool_version = tool_versions_string
            .lines()
            .find_map(|l| l.trim_start().strip_prefix("ruby "));

        if let Some(version) = tool_version {
            return Ok(Some((
                version.parse()?,
                Source::DotToolVersions(tool_versions),
            )));
        }
    }

    let lockfile = dir.join("Gemfile.lock");
    if lockfile.exists() {
        let raw_contents = std::fs::read_to_string(&lockfile)?;
        // Normalize Windows line endings (CRLF) to Unix (LF) for the parser
        let lockfile_contents = rv_lockfile::normalize_line_endings(&raw_contents);
        let lockfile_ruby = rv_lockfile::parse(&lockfile_contents)
            .map_err(Error::CouldNotParseGemfileLock)?
            .ruby_version;
        if let Some(lockfile_ruby) = lockfile_ruby {
            let version = ReleasedRubyVersion::from_gemfile_lock(lockfile_ruby)
                .map_err(Error::CouldNotParseGemfileLockVersion)?;
            return Ok(Some((version.into(), Source::GemfileLock(lockfile))));
        }
    }

    Ok(None)
}

const ENV_VARS: [&str; 8] = [
    "RUBY_ROOT",
    "RUBY_ENGINE",
    "RUBY_VERSION",
    "RUBYOPT",
    "GEM_ROOT",
    "GEM_HOME",
    "GEM_PATH",
    "MANPATH",
];

#[allow(clippy::type_complexity)]
pub fn env_for(ruby: Option<&Ruby>) -> Result<(Vec<&'static str>, Vec<(&'static str, String)>)> {
    env_with_path_for(ruby, Default::default())
}

#[allow(clippy::type_complexity)]
pub fn env_with_path_for(
    ruby: Option<&Ruby>,
    extra_paths: Vec<PathBuf>,
) -> Result<(Vec<&'static str>, Vec<(&'static str, String)>)> {
    let mut unset: Vec<_> = ENV_VARS.into();
    let mut set: Vec<(&'static str, String)> = vec![];

    let mut insert = |var: &'static str, val: String| {
        // PATH is never in the list to unset
        if let Some(i) = unset.iter().position(|i| *i == var) {
            unset.remove(i);
        }

        set.push((var, val));
    };

    let pathstr = env::var("PATH").unwrap_or_else(|_| String::new());
    let mut paths = split_paths(&pathstr).collect::<Vec<_>>();
    paths.extend(extra_paths);

    let old_ruby_paths: Vec<PathBuf> = ["RUBY_ROOT", "GEM_ROOT", "GEM_HOME"]
        .iter()
        .filter_map(|var| env::var(var).ok())
        .map(|p| std::path::Path::new(&p).join("bin"))
        .collect();

    let old_gem_paths: Vec<PathBuf> =
        env::var("GEM_PATH").map_or_else(|_| vec![], |p| split_paths(&p).collect::<Vec<_>>());

    // Remove old Ruby and Gem paths from PATH
    paths.retain(|p| !old_ruby_paths.contains(p) && !old_gem_paths.contains(p));

    if let Some(ruby) = ruby {
        let mut gem_paths = vec![];
        paths.insert(0, ruby.bin_path().into());
        insert("RUBY_ROOT", ruby.path.to_string());
        insert("RUBY_ENGINE", ruby.version.engine.name().into());
        insert("RUBY_VERSION", ruby.version.number());
        let gem_home = ruby.gem_home();
        paths.insert(0, gem_home.join("bin").into());
        gem_paths.insert(0, gem_home.clone());
        insert("GEM_HOME", gem_home.into_string());
        if let Some(gem_root) = ruby.gem_root() {
            paths.insert(0, gem_root.join("bin").into());
            gem_paths.insert(0, gem_root.clone());
            insert("GEM_ROOT", gem_root.into_string());
        }
        let gem_path = join_paths(gem_paths)?;
        if let Some(gem_path) = gem_path.to_str() {
            insert("GEM_PATH", gem_path.into());
        }

        // Set MANPATH so `man ruby`, `man irb`, etc. work correctly.
        // MANPATH is a Unix concept — Windows has no man page system.
        // A trailing colon means "also search system man directories".
        #[cfg(not(windows))]
        if let Some(man_path) = ruby.man_path() {
            let existing = env::var("MANPATH").unwrap_or_default();
            insert("MANPATH", format!("{}:{}", man_path, existing));
        }
    }

    let path = join_paths(paths)?;
    if let Some(path) = path.to_str() {
        insert("PATH", path.into());
    }

    Ok((unset, set))
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use indexmap::indexset;

    #[test]
    fn test_config() {
        let root = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        let ruby_dir = root.join("opt/rubies");
        std::fs::create_dir_all(&ruby_dir).unwrap();

        Config {
            ruby_dirs: indexset![ruby_dir],
            current_exe: root.join("bin").join("rv"),
            requested_ruby: RequestedRuby::Explicit("3.5.0".parse().unwrap()),
            cache: rv_cache::Cache::temp().unwrap(),
        };
    }

    #[test]
    fn test_default_ruby_dirs() {
        let root = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        default_ruby_dirs(&root);
    }
}
