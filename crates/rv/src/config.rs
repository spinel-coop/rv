use miette::Result;
use std::path::PathBuf;
use tracing::instrument;

use crate::ruby::Ruby;

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
                for entry in entries.flatten() {
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

        // Sort rubies by implementation and version
        rubies.sort();

        Ok(rubies)
    }
}

/// Default Ruby installation directories
pub fn default_ruby_dirs(root: &PathBuf) -> Vec<PathBuf> {
    // When root is "/" (production), use real system paths
    // When root is something else (testing), only look within the test root
    if root == &PathBuf::from("/") {
        vec![
            shellexpand::tilde("~/.rubies").as_ref(),
            "/opt/rubies",
            "/usr/local/rubies",
        ]
        .into_iter()
        .filter_map(|path| PathBuf::from(path).canonicalize().ok())
        .collect()
    } else {
        // For testing, only look within the test root directory
        vec![
            "Users/andre/.rubies", // corresponds to ~/.rubies in test
            "opt/rubies",
            "usr/local/rubies",
        ]
        .into_iter()
        .map(|path| root.join(path))
        .collect()
    }
}
