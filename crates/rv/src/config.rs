use camino::{Utf8Path, Utf8PathBuf};
use tracing::{debug, instrument};

use rv_ruby::{
    Ruby,
    request::{RequestError, RubyRequest},
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
}

type Result<T> = miette::Result<T, Error>;

#[derive(Debug)]
pub struct Config {
    pub ruby_dirs: Vec<Utf8PathBuf>,
    pub gemfile: Option<Utf8PathBuf>,
    pub root: Utf8PathBuf,
    pub current_dir: Utf8PathBuf,
    pub project_dir: Option<Utf8PathBuf>,
    pub cache: rv_cache::Cache,
}

impl Config {
    #[instrument(skip_all)]
    pub fn rubies(&self) -> Vec<Ruby> {
        self.discover_rubies()
    }

    pub fn requested_ruby(&self) -> Result<RubyRequest> {
        if let Some(project_dir) = &self.project_dir {
            let rv_file = project_dir.join(".ruby-version");

            std::fs::read_to_string(&rv_file)
                .map_err(Error::from)
                .and_then(|s| Ok(s.parse::<RubyRequest>()?))
        } else {
            Ok(RubyRequest::default())
        }
    }
}

/// Default Ruby installation directories
pub fn default_ruby_dirs(root: &Utf8Path) -> Vec<Utf8PathBuf> {
    vec![
        shellexpand::tilde("~/.rubies").as_ref(),
        "/opt/rubies",
        "/usr/local/rubies",
    ]
    .into_iter()
    .filter_map(|path| {
        let joinable_path = path.strip_prefix("/").unwrap();
        let joined_path = root.join(joinable_path);
        // Make sure we always have at least ~/.rubies, even if it doesn't exist yet
        if joined_path.ends_with(".rubies") {
            Some(joined_path)
        } else {
            joined_path.canonicalize_utf8().ok()
        }
    })
    .collect()
}
pub fn find_project_dir(current_dir: Utf8PathBuf, root: Utf8PathBuf) -> Option<Utf8PathBuf> {
    debug!("Searching for project directory in {}", current_dir);
    let mut project_dir = current_dir.clone();
    while project_dir != root {
        let ruby_version = project_dir.join(".ruby-version");
        if ruby_version.exists() {
            debug!("Found project directory {}", project_dir);
            return Some(project_dir);
        }
        project_dir = project_dir.parent().unwrap_or_else(|| &root).into();
    }
    debug!("No project directory found in parents of {}", current_dir);
    None
}
