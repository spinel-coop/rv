use miette::Result;
use rsfs::*;
use std::path::PathBuf;
use tracing::instrument;

use crate::ruby::Ruby;

#[derive(Debug)]
pub struct Config<F: GenFS> {
    pub ruby_dirs: Vec<PathBuf>,
    pub gemfile: Option<PathBuf>,
    pub root: F,
    pub current_dir: PathBuf,
    pub project_dir: Option<PathBuf>,
}

impl<F: GenFS> Config<F> {
    #[instrument(skip_all)]
    pub fn rubies(&self) -> Result<Vec<Ruby>> {
        let mut rubies = Vec::new();

        for ruby_dir in &self.ruby_dirs {
            if self.root.metadata(ruby_dir).is_err() {
                continue;
            }

            if let Ok(entries) = self.root.read_dir(ruby_dir) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if let Ok(metadata) = entry.metadata()
                        && metadata.is_dir()
                        && let Ok(ruby) = Ruby::from_dir(&self.root, &entry_path)
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
}

/// Default Ruby installation directories
pub fn default_ruby_dirs() -> Vec<PathBuf> {
    vec![
        shellexpand::tilde("~/.rubies").as_ref(),
        "/opt/rubies",
        "/usr/local/rubies",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect()
}
