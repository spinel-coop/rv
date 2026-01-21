use std::{
    env::{self, JoinPathsError, join_paths, split_paths},
    path::{Path, PathBuf},
    str::FromStr,
};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use tracing::{debug, instrument};

use rv_ruby::{
    Ruby,
    request::{RequestError, RubyRequest, Source},
    version::{ParseVersionError, RubyVersion},
};

mod ruby_cache;
mod ruby_fetcher;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("Ruby cache miss or invalid cache for {}", ruby_path)]
    RubyCacheMiss { ruby_path: Utf8PathBuf },
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    RequestError(#[from] RequestError),
    #[error(transparent)]
    EnvError(#[from] std::env::VarError),
    #[error(transparent)]
    JoinPathsError(#[from] JoinPathsError),
    #[error("Tried to parse ruby version from Gemfile.lock, but that file was invalid: {0}")]
    CouldNotParseGemfileLock(#[from] rv_lockfile::ParseErrors),
    #[error("Could not parse ruby version from Gemfile.lock: {0}")]
    CouldNotParseGemfileLockVersion(ParseVersionError),
}

type Result<T> = miette::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Config {
    pub ruby_dirs: IndexSet<Utf8PathBuf>,
    pub root: Utf8PathBuf,
    pub current_dir: Utf8PathBuf,
    pub cache: rv_cache::Cache,
    pub current_exe: Utf8PathBuf,
    pub requested_ruby: Option<(RubyRequest, Source)>,
}

impl Config {
    #[instrument(skip_all, level = "trace")]
    pub fn rubies(&self) -> Vec<Ruby> {
        self.discover_installed_rubies()
    }

    pub async fn remote_rubies(&self) -> Vec<Ruby> {
        self.discover_remote_rubies().await
    }

    pub fn matching_ruby(&self, request: &RubyRequest) -> Option<Ruby> {
        self.discover_rubies_matching(|dir_name| {
            RubyVersion::from_str(dir_name).is_ok_and(|v| v.satisfies(request))
        })
        .last()
        .cloned()
    }

    pub fn current_ruby(&self) -> Option<Ruby> {
        self.matching_ruby(&self.ruby_request())
    }

    pub fn ruby_request(&self) -> RubyRequest {
        if let Some(request) = &self.requested_ruby {
            request.0.clone()
        } else {
            RubyRequest::default()
        }
    }
}

fn xdg_data_path() -> String {
    let xdg_data_home =
        env::var("XDG_DATA_HOME").unwrap_or(shellexpand::tilde("~/.local/share").into());
    let path_buf = Path::new(&xdg_data_home).join("rv/rubies");
    path_buf.to_str().unwrap().to_owned()
}

fn legacy_default_data_path() -> String {
    shellexpand::tilde("~/.data/rv/rubies").into()
}

fn legacy_default_path() -> String {
    shellexpand::tilde("~/.rubies").into()
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
            join.canonicalize_utf8()
                .ok()
                .or(always_include.then_some(path.into()))
        })
        .collect()
}

pub fn find_requested_ruby(
    current_dir: Utf8PathBuf,
    root: Utf8PathBuf,
) -> Result<Option<(RubyRequest, Source)>> {
    debug!("Searching for project directory in {}", current_dir);
    let mut project_dir = current_dir;

    loop {
        let ruby_version = project_dir.join(".ruby-version");
        if ruby_version.exists() {
            debug!("Found project directory {}", project_dir);
            let ruby_version_string = std::fs::read_to_string(&ruby_version)?;
            return Ok(Some((
                ruby_version_string.parse()?,
                Source::DotRubyVersion(ruby_version),
            )));
        }

        let tools_versions = project_dir.join(".tool-versions");
        if tools_versions.exists() {
            let tools_versions_string = std::fs::read_to_string(&tools_versions)?;
            let tools_version = tools_versions_string
                .lines()
                .find_map(|l| l.trim_start().strip_prefix("ruby "));

            if let Some(version) = tools_version {
                return Ok(Some((
                    version.parse()?,
                    Source::DotToolVersions(tools_versions),
                )));
            }
        }

        let lockfile = project_dir.join("Gemfile.lock");
        if lockfile.exists() {
            let lockfile_contents = std::fs::read_to_string(&lockfile)?;
            let lockfile_ruby = rv_lockfile::parse(&lockfile_contents)
                .map_err(Error::CouldNotParseGemfileLock)?
                .ruby_version;
            if let Some(lockfile_ruby) = lockfile_ruby {
                tracing::debug!("Found ruby {lockfile_ruby} in Gemfile.lock");
                let version = RubyVersion::from_gemfile_lock(lockfile_ruby)
                    .map_err(Error::CouldNotParseGemfileLockVersion)?;
                return Ok(Some((version.into(), Source::GemfileLock(lockfile))));
            }
        }

        if project_dir == root {
            debug!("Reached root {} without finding a project directory", root);
            return Ok(None);
        }

        if let Some(parent_dir) = project_dir.parent() {
            project_dir = parent_dir.to_owned();
        } else {
            debug!(
                "Ran out of parents of {} without finding a project directory",
                project_dir
            );
            return Ok(None);
        }
    }
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
    let mut unset: Vec<_> = ENV_VARS.into();
    let mut set: Vec<(&'static str, String)> = vec![];

    let mut insert = |var: &'static str, val: String| {
        // PATH is never in the list to unset
        if let Some(i) = unset.iter().position(|i| *i == var) {
            unset.remove(i);
        }

        set.push((var, val));
    };

    let pathstr = std::env::var("PATH").unwrap_or_else(|_| String::new());
    let mut paths = split_paths(&pathstr).collect::<Vec<_>>();

    let old_ruby_paths: Vec<PathBuf> = ["RUBY_ROOT", "GEM_ROOT", "GEM_HOME"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .map(|p| std::path::Path::new(&p).join("bin"))
        .collect();

    let old_gem_paths: Vec<PathBuf> =
        std::env::var("GEM_PATH").map_or_else(|_| vec![], |p| split_paths(&p).collect::<Vec<_>>());

    // Remove old Ruby and Gem paths from PATH
    paths.retain(|p| !old_ruby_paths.contains(p) && !old_gem_paths.contains(p));

    if let Some(ruby) = ruby {
        let mut gem_paths = vec![];
        paths.insert(0, ruby.bin_path().into());
        insert("RUBY_ROOT", ruby.path.to_string());
        insert("RUBY_ENGINE", ruby.version.engine.name().into());
        insert("RUBY_VERSION", ruby.version.number());
        if let Some(gem_home) = ruby.gem_home() {
            paths.insert(0, gem_home.join("bin").into());
            gem_paths.insert(0, gem_home.clone());
            insert("GEM_HOME", gem_home.into_string());
        }
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
        // A trailing colon means "also search system man directories".
        if let Some(man_path) = ruby.man_path() {
            let existing = std::env::var("MANPATH").unwrap_or_default();
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
        let current_dir = root.join("project");
        std::fs::create_dir_all(&current_dir).unwrap();

        Config {
            ruby_dirs: indexset![ruby_dir],
            current_exe: root.join("bin").join("rv"),
            requested_ruby: Some(("3.5.0".parse().unwrap(), Source::Other)),
            current_dir,
            cache: rv_cache::Cache::temp().unwrap(),
            root,
        };
    }

    #[test]
    fn test_default_ruby_dirs() {
        let root = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        default_ruby_dirs(&root);
    }

    #[test]
    fn test_find_requested_ruby() {
        let root = Utf8PathBuf::from(TempDir::new().unwrap().path().to_str().unwrap());
        find_requested_ruby(root.clone(), root).unwrap();
    }
}
