use camino::{Utf8Path, Utf8PathBuf};
use tracing::instrument;

use rv_ruby::Ruby;

mod ruby_cache;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("No project was found in the parents of {}", current_dir)]
    NoProjectDir { current_dir: Utf8PathBuf },
    #[error("Ruby cache miss or invalid cache for {}", ruby_path)]
    RubyCacheMiss { ruby_path: Utf8PathBuf },
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

    pub fn get_project_dir(&self) -> Result<&Utf8PathBuf> {
        match self.project_dir {
            None => Err(Error::NoProjectDir {
                current_dir: self.current_dir.clone(),
            }),
            Some(ref dir) => Ok(dir),
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
