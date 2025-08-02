use miette::{Diagnostic, Result};
use std::path::PathBuf;
use tracing::instrument;

use rv_ruby::Ruby;

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum Error {
    #[error("No project was found in the parents of {}", current_dir.to_string_lossy())]
    NoProjectDir { current_dir: PathBuf },
}

#[derive(Debug)]
pub struct Config {
    pub ruby_dirs: Vec<PathBuf>,
    pub gemfile: Option<PathBuf>,
    pub root: PathBuf,
    pub current_dir: PathBuf,
    pub project_dir: Option<PathBuf>,
}

impl Config {
    #[instrument(skip_all)]
    pub fn rubies(&self) -> Result<Vec<Ruby>> {
        let mut rubies = Vec::new();

        for ruby_dir in &self.ruby_dirs {
            if !ruby_dir.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(ruby_dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Ok(metadata) = entry.metadata()
                            && metadata.is_dir()
                            && let Ok(ruby) = Ruby::from_dir(entry.path())
                            && ruby.is_valid()
                        {
                            rubies.push(ruby);
                        }
                    }
                }
            }
        }

        // Sort rubies by implementation and version
        rubies.sort();

        Ok(rubies)
    }

    pub fn get_project_dir(&self) -> Result<&PathBuf, Error> {
        match self.project_dir {
            None => Err(Error::NoProjectDir {
                current_dir: self.current_dir.clone(),
            }),
            Some(ref dir) => Ok(dir),
        }
    }
}

/// Default Ruby installation directories
pub fn default_ruby_dirs(root: &PathBuf) -> Vec<PathBuf> {
    vec![
        shellexpand::tilde("~/.rubies").as_ref(),
        "/opt/rubies",
        "/usr/local/rubies",
    ]
    .into_iter()
    .filter_map(|path| {
        let full_path = root.join(path);
        if full_path.starts_with(root) {
            Some(full_path)
        } else {
            PathBuf::from(path).canonicalize().ok()
        }
    })
    .collect()
}
