use std::{
    env::{self, JoinPathsError, join_paths, split_paths},
    path::PathBuf,
    str::FromStr,
};

use bundler_settings::Error as BundlerSettingsError;
use rv_settings::Error as RvSettingsError;

use bundler_settings::BundlerSettings;
use camino::{FromPathBufError, Utf8Path, Utf8PathBuf};
use indexmap::IndexSet;
use rv_settings::RvSettings;
use tracing::{debug, error, instrument};

use rv_ruby::{
    EnvProvider, RemoteRuby, Ruby, SystemEnv,
    request::{RequestError, RubyRequest, Source},
    version::RubyVersion,
};

use rv_gem_types::Requirement;

use crate::GlobalArgs;
use crate::update;

pub mod bundler_settings;
pub mod github;
mod ruby_cache;
mod ruby_fetcher;
pub mod rv_settings;
mod system_ruby;

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::HashMap;

    use rv_ruby::EnvProvider;

    #[derive(Default)]
    pub struct FakeEnv {
        vars: HashMap<String, String>,
    }

    impl FakeEnv {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with(mut self, key: &str, value: &str) -> Self {
            self.vars.insert(key.to_string(), value.to_string());
            self
        }
    }

    impl EnvProvider for FakeEnv {
        fn get_var(&self, key: &str) -> Option<String> {
            self.vars.get(key).cloned()
        }
    }

    /// Writes a mock ruby at `<dir>/bin/ruby` that emits the metadata
    /// expected by `extract_ruby_info`. Used by `system_ruby` tests.
    pub fn make_mock_ruby_shim(dir: &std::path::Path) -> std::path::PathBuf {
        use std::fs;
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let exec = bin.join("ruby");
        let script = "\
#!/bin/bash
 echo \"ruby\"\n echo \"3.0.1\"\n echo \"aarch64-darwin23\"\n echo \"aarch64\"\n echo \"darwin23\"\n echo \"\"\n";
        fs::write(&exec, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&exec).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&exec, perms).unwrap();
        }
        exec
    }
}

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
    #[error(transparent)]
    RvSettingsError(#[from] RvSettingsError),
    #[error(transparent)]
    BundlerSettingsError(#[from] BundlerSettingsError),
    #[error("no matching ruby version found")]
    NoMatchingRuby,
    #[error(
        "No available Ruby matched the Ruby requirements. The requirements were {requirement:?}"
    )]
    NoRubyMatchingRequirement { requirement: Requirement },
}

type Result<T> = miette::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Config {
    pub ruby_dirs: IndexSet<Utf8PathBuf>,
    pub project_root: Utf8PathBuf,
    pub cache: rv_cache::Cache,
    pub requested_ruby: RequestedRuby,
    pub bundler_settings: BundlerSettings,
    pub rv_settings: RvSettings,
    pub offline: bool,
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

impl Config {
    pub(crate) fn new(global_args: &GlobalArgs, request: Option<RubyRequest>) -> Result<Self> {
        let root = rv_dirs::root_dir();
        let ruby_dirs = rv_dirs::canonical_ruby_dirs(&global_args.ruby_dir, &root)?;
        let cache = global_args.cache_args.to_cache()?;

        let project_root = rv_dirs::project_root(&root)?;
        debug!("Found project directory in {}", project_root);

        let home_dir = rv_dirs::home_dir();

        let requested_ruby = RequestedRuby::new(request, &home_dir, &project_root)?;
        let bundler_settings = BundlerSettings::default();
        let rv_settings = RvSettings::default();
        let offline = global_args.offline;

        Ok(Self {
            ruby_dirs,
            project_root,
            cache,
            requested_ruby,
            bundler_settings,
            rv_settings,
            offline,
        })
    }

    pub(crate) fn with_settings(
        global_args: &GlobalArgs,
        request: Option<RubyRequest>,
    ) -> Result<Self> {
        let mut config = Self::new(global_args, request)?;
        let home_dir = rv_dirs::home_dir();

        config.bundler_settings = BundlerSettings::new(&home_dir, &config.project_root)
            .inspect_err(|err| error!("{}", err))
            .unwrap_or_default();
        config.rv_settings = RvSettings::new(global_args, &home_dir, &config.project_root)?;

        Ok(config)
    }

    pub async fn self_update_if_needed(&self) {
        update::check(&self.rv_settings.update_mode).await;
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
        // `TempDir` deletes the directory on drop. Persist it for the
        // lifetime of the test process — the OS temp cleaner will reclaim it.
        std::mem::forget(temp_dir);

        Self {
            ruby_dirs: indexset![ruby_dir],
            project_root: root,
            cache: Cache::temp().unwrap(),
            requested_ruby: RequestedRuby::Global,
            bundler_settings: BundlerSettings::default(),
            rv_settings: RvSettings::default(),
            offline: false,
        }
    }

    #[instrument(skip_all, level = "trace")]
    pub fn rubies(&self) -> Vec<Ruby> {
        let mut rubies = self.discover_installed_rubies();
        if include_system_rubies() {
            rubies.extend(self.discover_system_rubies());
            rubies.sort();
        }
        rubies
    }

    /// Like [`Self::rubies`], but applies `predicate` to the version string of
    /// each candidate. Used by [`Self::highest_ruby_matching`] for version
    /// pinning and `uninstall`. Includes system Rubies when discovery is
    /// enabled so the uninstall safety check (#762) can fire.
    fn rubies_with_filter<F>(&self, predicate: F) -> Vec<Ruby>
    where
        F: Fn(&str) -> bool + Clone,
    {
        let mut rubies = self.discover_installed_rubies_matching(&predicate);
        if include_system_rubies() {
            rubies.extend(self.discover_system_rubies_filtered(&predicate));
            rubies.sort();
        }
        rubies
    }

    pub async fn remote_rubies(&self) -> Vec<RemoteRuby> {
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

            Ok(matched_ruby.version)
        }
    }

    pub fn best_ruby(&self) -> Option<Ruby> {
        self.current_ruby()
            .or_else(|| self.highest_ruby_matching(&RubyRequest::default()))
    }

    pub async fn best_ruby_matching_requirement(
        &self,
        requirement: &Requirement,
    ) -> Result<RubyVersion> {
        let installed_rubies = self.rubies();

        match requirement.find_match_in(&installed_rubies, false) {
            Some(local_ruby) => Ok(local_ruby.version),
            None => {
                let remote_rubies = &self.remote_rubies().await;

                match requirement
                    .find_match_in(remote_rubies, false)
                    .or_else(|| requirement.find_match_in(remote_rubies, true))
                {
                    Some(remote_ruby) => Ok(remote_ruby.version),
                    None => Err(Error::NoRubyMatchingRequirement {
                        requirement: requirement.clone(),
                    }),
                }
            }
        }
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
        if let Some(install_path) = &self.rv_settings.install_path_as_utf8pathbuf() {
            return install_path.join(ruby.gem_scope());
        }

        if let Some(path) = self.bundler_settings.path() {
            return path.join(ruby.gem_scope());
        }

        ruby.gem_home()
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
        self.rubies_with_filter(|dir_name| {
            if dir_name == "ruby-dev" {
                request.is_dev()
            } else {
                RubyVersion::from_str(dir_name).is_ok_and(|v| v.satisfies(request))
            }
        })
        .last()
        .cloned()
    }
}

/// Returns whether to surface system Rubies (Debian `/usr/bin/ruby`, etc.) in
/// `rv ruby list` and friends. Defaults to `true`. Set `RV_INCLUDE_SYSTEM_RUBY=0`
/// (or `false`) to disable — useful for CI that wants only `rv`-managed rubies.
fn include_system_rubies_with<E: EnvProvider>(env: &E) -> bool {
    match env.get_var("RV_INCLUDE_SYSTEM_RUBY") {
        Some(val) => !matches!(val.to_ascii_lowercase().as_str(), "0" | "false" | "no" | "off"),
        None => true,
    }
}

fn include_system_rubies() -> bool {
    include_system_rubies_with(&SystemEnv)
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
                return Ok(Some((
                    lockfile_ruby.cruby_version.into(),
                    Source::GemfileLock(lockfile),
                )));
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
#[cfg(test)]
mod tests {
    use super::include_system_rubies_with;
    use crate::config::test_support::FakeEnv;

    #[test]
    fn include_system_rubies_defaults_true_when_unset() {
        let env = FakeEnv::default();
        assert!(include_system_rubies_with(&env));
    }

    #[test]
    fn include_system_rubies_truthy_values() {
        for v in ["1", "true", "yes", "on", "TRUE", "Yes", "1"] {
            let env = FakeEnv::default().with("RV_INCLUDE_SYSTEM_RUBY", v);
            assert!(include_system_rubies_with(&env), "expected true for {v:?}");
        }
    }

    #[test]
    fn include_system_rubies_falsy_values() {
        for v in ["0", "false", "no", "off", "FALSE", "No", "OFF"] {
            let env = FakeEnv::default().with("RV_INCLUDE_SYSTEM_RUBY", v);
            assert!(
                !include_system_rubies_with(&env),
                "expected false for {v:?}",
            );
        }
    }
}
