use std::{
    env::{self, JoinPathsError, join_paths, split_paths},
    path::{Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use tracing::{debug, instrument};

use rv_ruby::{
    Ruby,
    request::{RequestError, RubyRequest, Source},
};

mod ruby_cache;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("No project was found in the parents of {}", current_dir)]
    NoProjectDir { current_dir: Utf8PathBuf },
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
}

type Result<T> = miette::Result<T, Error>;

#[derive(Debug)]
pub struct Config {
    pub ruby_dirs: IndexSet<Utf8PathBuf>,
    pub gemfile: Option<Utf8PathBuf>,
    pub root: Utf8PathBuf,
    pub current_dir: Utf8PathBuf,
    pub cache: rv_cache::Cache,
    pub current_exe: Utf8PathBuf,
    pub requested_ruby: Option<(RubyRequest, Source)>,
}

impl Config {
    #[instrument(skip_all)]
    pub fn rubies(&self) -> Vec<Ruby> {
        self.discover_rubies()
    }

    pub fn matching_ruby(&self, request: &RubyRequest) -> Option<Ruby> {
        let rubies = self.rubies();
        rubies
            .into_iter()
            .rev()
            .find(|ruby| request.satisfied_by(ruby))
    }

    pub fn current_ruby(&self) -> Option<Ruby> {
        if let Ok(request) = self.ruby_request() {
            self.matching_ruby(&request)
        } else {
            None
        }
    }

    pub fn ruby_request(&self) -> Result<RubyRequest> {
        if let Some(request) = &self.requested_ruby {
            Ok(request.0.clone())
        } else {
            Ok(RubyRequest::default())
        }
    }
}

fn xdg_env_var_path() -> Option<String> {
    let xdg_data_home = env::var("XDG_DATA_HOME").ok()?;
    let path_buf = Path::new(&xdg_data_home).join("rv/rubies");
    Some(path_buf.to_str()?.to_owned())
}

struct PathInfo<'a> {
    path: &'a str,
    // Always include, even if it doesn't exist yet.
    always_include: bool,
}

impl<'a> PathInfo<'a> {
    pub fn new(path: &'a str, always_include: bool) -> Self {
        Self {
            path,
            always_include,
        }
    }
}

/// Default Ruby installation directories
pub fn default_ruby_dirs(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    let mut paths: Vec<PathInfo> = vec![];
    let xdg_path = xdg_env_var_path();
    if let Some(xdg_path) = &xdg_path {
        paths.push(PathInfo::new(xdg_path, true));
    }
    let default_path = shellexpand::tilde("~/.data/rv/rubies");
    paths.push(PathInfo::new(default_path.as_ref(), true));

    // TODO: The paths below are never used, even if they exist (and the ones above don't).
    let legacy_default_path = shellexpand::tilde("~/.rubies");
    paths.push(PathInfo::new(legacy_default_path.as_ref(), false));
    paths.push(PathInfo::new("/opt/rubies", false));
    paths.push(PathInfo::new("/usr/local/rubies", false));

    paths
        .into_iter()
        .filter_map(|path_info| {
            let joinable_path = path_info.path.strip_prefix("/").unwrap();
            let joined_path = root.join(joinable_path);
            if path_info.always_include {
                Some(joined_path)
            } else {
                joined_path.canonicalize_utf8().ok()
            }
        })
        .collect()
}

pub fn find_requested_ruby(
    current_dir: Utf8PathBuf,
    root: Utf8PathBuf,
) -> Result<Option<(RubyRequest, Source)>> {
    debug!("Searching for project directory in {}", current_dir);
    let mut project_dir = current_dir.clone();

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
            let mut tools_version = None;

            for line in tools_versions_string.lines() {
                if line.trim_start().starts_with("ruby") {
                    tools_version = line.trim_start().strip_prefix("ruby")
                }
            }

            if let Some(version) = tools_version {
                return Ok(Some((
                    version.parse()?,
                    Source::DotToolVersions(tools_versions),
                )));
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

const ENV_VARS: [&str; 7] = [
    "RUBY_ROOT",
    "RUBY_ENGINE",
    "RUBY_VERSION",
    "RUBYOPT",
    "GEM_ROOT",
    "GEM_HOME",
    "GEM_PATH",
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
    }

    let path = join_paths(paths)?;
    if let Some(path) = path.to_str() {
        insert("PATH", path.into());
    }

    Ok((unset, set))
}
