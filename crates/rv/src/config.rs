use std::{
    env::{self, JoinPathsError, join_paths, split_paths},
    path::PathBuf,
    str::FromStr,
};

use bundler_settings::BundlerSettings;
use camino::{FromPathBufError, Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use tracing::{debug, instrument};

use rv_ruby::{
    Ruby,
    request::{RequestError, RubyRequest, Source},
    version::{ReleasedRubyVersion, RubyVersion},
};

use crate::GlobalArgs;

pub mod bundler_settings;
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
    JoinPathsError(#[from] JoinPathsError),
    #[error("no matching ruby version found")]
    NoMatchingRuby,
}

type Result<T> = miette::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Config<'input> {
    pub ruby_dirs: IndexSet<Utf8PathBuf>,
    pub project_root: Utf8PathBuf,
    pub cache: rv_cache::Cache,
    pub current_exe: Utf8PathBuf,
    pub requested_ruby: RequestedRuby,
    pub bundler_settings: BundlerSettings<'input>,
}

#[derive(Debug, Clone)]
pub enum RequestedRuby {
    Explicit(RubyRequest),
    Project((RubyRequest, Source)),
    User((RubyRequest, Source)),
    Global,
}

impl RequestedRuby {
    pub fn new(
        request: Option<RubyRequest>,
        home_dir: &Utf8PathBuf,
        project_root: &Utf8PathBuf,
    ) -> Result<Self> {
        let requested_ruby = match request {
            Some(req) => {
                debug!("Explicit ruby request for {} received", req);
                Self::Explicit(req)
            }
            None => {
                if let Some(req) = find_directory_ruby(project_root)? {
                    debug!("Found project ruby request for {} in {:?}", req.0, req.1);
                    Self::Project(req)
                } else if let Some(req) = find_directory_ruby(home_dir)? {
                    debug!("Found user ruby request for {} in {:?}", req.0, req.1);
                    Self::User(req)
                } else {
                    Self::Global
                }
            }
        };

        Ok(requested_ruby)
    }

    pub fn explain(&self, installed: bool) -> String {
        match self {
            Self::Explicit(_) => "* Default version explicitly selected".to_string(),
            Self::Project((_, source)) => format!(
                "* Default version pinned by {}",
                rv_dirs::relativize(source.path())
            ),
            Self::User((_, source)) => format!(
                "* Default version pinned by {}",
                rv_dirs::unexpand(source.path())
            ),
            Self::Global => {
                let installed_or_available = if installed { "installed" } else { "available" };
                format!("* Default version is the latest {installed_or_available}")
            }
        }
    }
}

impl Config<'_> {
    pub(crate) fn new(global_args: &GlobalArgs, request: Option<RubyRequest>) -> Result<Self> {
        let root = rv_dirs::root_dir();
        let ruby_dirs = rv_dirs::canonical_ruby_dirs(&global_args.ruby_dir, &root)?;
        let cache = global_args.cache_args.to_cache()?;
        let current_exe = if let Some(exe) = global_args.current_exe.clone() {
            exe
        } else {
            std::env::current_exe()?.to_str().unwrap().into()
        };

        let project_root = rv_dirs::project_root(&root)?;
        debug!("Found project directory in {}", project_root);

        let home_dir = rv_dirs::home_dir();

        let requested_ruby = RequestedRuby::new(request, &home_dir, &project_root)?;
        let bundler_settings = BundlerSettings::new(&home_dir, &project_root);

        Ok(Self {
            ruby_dirs,
            project_root,
            cache,
            current_exe,
            requested_ruby,
            bundler_settings,
        })
    }

    #[cfg(test)]
    pub fn new_dummy() -> Self {
        use assert_fs::TempDir;
        use indexmap::indexset;
        use rv_cache::Cache;
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let root = Utf8PathBuf::from(temp_dir.path().to_str().unwrap());
        let ruby_dir = root.join("rubies");
        fs::create_dir_all(&ruby_dir).unwrap();

        let home_dir = root.join("home");
        let project_dir = root.join("project");

        Self {
            bundler_settings: BundlerSettings::new(&home_dir, &project_dir),
            ruby_dirs: indexset![ruby_dir],
            project_root: root.clone(),
            cache: Cache::temp().unwrap(),
            current_exe: root.join("bin").join("rv"),
            requested_ruby: RequestedRuby::Global,
        }
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

    pub fn best_ruby(&self) -> Option<Ruby> {
        self.current_ruby()
            .or_else(|| self.highest_ruby_matching(&RubyRequest::default()))
    }

    pub fn current_ruby(&self) -> Option<Ruby> {
        self.highest_ruby_matching(&self.ruby_request())
    }

    pub fn ruby_request(&self) -> RubyRequest {
        match &self.requested_ruby {
            RequestedRuby::Explicit(request) => request.clone(),
            RequestedRuby::Project((request, _)) => request.clone(),
            RequestedRuby::User((request, _)) => request.clone(),
            RequestedRuby::Global => RubyRequest::default(),
        }
    }

    pub fn is_requested_ruby_installed_in_dir(&self, install_root: &Utf8Path) -> bool {
        let requested_ruby_name = self.ruby_request().to_string();

        let install_path = install_root.join(requested_ruby_name);

        let managed = self.ruby_dirs.first().is_some_and(|d| *d == *install_root);

        Ruby::from_dir(install_path, managed)
            .map(|ruby| ruby.is_valid())
            .unwrap_or(false)
    }

    pub fn gem_home(&self, ruby: &Ruby) -> Utf8PathBuf {
        self.bundler_settings
            .path()
            .map(|p| p.join(ruby.gem_scope()))
            .unwrap_or(ruby.gem_home())
    }

    pub fn env_for(&self, ruby: Option<&Ruby>) -> Result<Env> {
        self.env_with_path_for(ruby, Default::default())
    }

    pub fn env_with_path_for(&self, ruby: Option<&Ruby>, extra_paths: Vec<PathBuf>) -> Result<Env> {
        let mut env = Env::default();

        let pathstr = env::var("PATH").unwrap_or_else(|_| String::new());
        let mut paths = split_paths(&pathstr).collect::<IndexSet<_>>();
        for extra_path in extra_paths {
            paths.insert(extra_path);
        }

        let old_ruby_paths: Vec<PathBuf> = ["RUBY_ROOT", "GEM_HOME"]
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
            paths.insert_before(0, ruby.bin_path().into());
            env.insert("RUBY_ROOT", ruby.path.to_string());
            env.insert("RUBY_ENGINE", ruby.version.engine.name().into());
            env.insert("RUBY_VERSION", ruby.version.number());
            let gem_home = self.gem_home(ruby);
            paths.insert_before(0, gem_home.join("bin").into());
            gem_paths.insert(0, gem_home.clone());
            env.insert("GEM_HOME", gem_home.into_string());
            let user_home = ruby.user_home();
            paths.insert_before(0, user_home.join("bin").into());
            gem_paths.insert(0, user_home);
            let gem_path = join_paths(gem_paths)?;
            if let Some(gem_path) = gem_path.to_str() {
                env.insert("GEM_PATH", gem_path.into());
            }

            // Set MANPATH so `man ruby`, `man irb`, etc. work correctly.
            // MANPATH is a Unix concept — Windows has no man page system.
            // A trailing colon means "also search system man directories".
            #[cfg(not(windows))]
            if let Some(man_path) = ruby.man_path() {
                let existing = env::var("MANPATH").unwrap_or_default();
                let man_paths = split_paths(&existing).collect::<Vec<_>>();

                if !man_paths.contains(&man_path.to_path_buf().into_std_path_buf()) {
                    env.insert("MANPATH", format!("{}:{}", man_path, existing));
                }
            }
        }

        let path = join_paths(paths)?;
        if let Some(path) = path.to_str() {
            env.insert("PATH", path.into());
        }

        Ok(env)
    }

    fn highest_ruby_matching(&self, request: &RubyRequest) -> Option<Ruby> {
        self.discover_rubies_matching(|dir_name| {
            let version_res = RubyVersion::from_str(dir_name);
            version_res.is_ok_and(|v| v.satisfies(request))
        })
        .last()
        .cloned()
    }
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

        if let Ok(parsed_lockfile) = rv_lockfile::parse(&lockfile_contents) {
            let lockfile_ruby = parsed_lockfile.ruby_version;

            if let Some(lockfile_ruby) = lockfile_ruby {
                if let Ok(version) = ReleasedRubyVersion::from_gemfile_lock(lockfile_ruby.content())
                {
                    return Ok(Some((version.into(), Source::GemfileLock(lockfile))));
                } else {
                    debug!(
                        "Ignoring ruby version in {} because it could not be parsed",
                        lockfile
                    );
                }
            }
        } else {
            debug!(
                "Ignoring {} while discovering ruby version to use because it could not be parsed",
                lockfile
            );
        }
    }

    Ok(None)
}

pub struct Env {
    unset: Vec<&'static str>,

    set: Vec<(&'static str, String)>,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            set: vec![],
            unset: Self::ENV_VARS.into(),
        }
    }
}

impl Env {
    const ENV_VARS: [&str; 6] = [
        "RUBY_ROOT",
        "RUBY_ENGINE",
        "RUBY_VERSION",
        "RUBYOPT",
        "GEM_HOME",
        "GEM_PATH",
    ];

    pub fn insert(&mut self, var: &'static str, val: String) {
        // PATH is never in the list to unset
        if let Some(i) = self.unset.iter().position(|i| *i == var) {
            self.unset.remove(i);
        }

        self.set.push((var, val));
    }

    pub fn split(&self) -> (Vec<&'static str>, Vec<(&'static str, String)>) {
        (self.unset.clone(), self.set.clone())
    }
}
