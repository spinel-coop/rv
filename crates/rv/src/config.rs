use camino::{Utf8Path, Utf8PathBuf};
use miette::{Diagnostic, Result};
use tracing::instrument;

use rv_ruby::Ruby;

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum Error {
    #[error("No project was found in the parents of {}", current_dir)]
    NoProjectDir { current_dir: Utf8PathBuf },
}

#[derive(Debug)]
pub struct Config {
    pub ruby_dirs: Vec<Utf8PathBuf>,
    pub gemfile: Option<Utf8PathBuf>,
    pub root: Utf8PathBuf,
    pub current_dir: Utf8PathBuf,
    pub project_dir: Option<Utf8PathBuf>,
}

impl Config {
    #[instrument(skip_all)]
    pub fn rubies(&self) -> Result<Vec<Ruby>> {
        let mut rubies = Vec::new();

        for ruby_dir in &self.ruby_dirs {
            if !ruby_dir.exists() {
                continue;
            }

            if let Ok(entries) = ruby_dir.read_dir_utf8() {
                for entry in entries {
                    if let Ok(entry) = entry
                        && let Ok(metadata) = entry.metadata()
                        && metadata.is_dir()
                        && let Ok(ruby) = Ruby::from_dir(entry.into_path())
                        && ruby.is_valid()
                    {
                        rubies.push(ruby);
                    }
                }
            }
        }

        // Sort rubies by implementation and version
        rubies.sort();

        Ok(rubies)
    }

    pub fn get_project_dir(&self) -> Result<&Utf8PathBuf, Error> {
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
