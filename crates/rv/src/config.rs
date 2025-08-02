use miette::{Diagnostic, Result};
use std::path::PathBuf;
use tracing::instrument;
use vfs::VfsPath;

use rv_ruby::Ruby;

#[derive(Debug, thiserror::Error, Diagnostic)]
pub enum Error {
    #[error("No project was found in the parents of {}", current_dir.as_str())]
    NoProjectDir { current_dir: VfsPath },
}

#[derive(Debug)]
pub struct Config {
    pub ruby_dirs: Vec<VfsPath>,
    pub gemfile: Option<PathBuf>,
    pub root: VfsPath,
    pub current_dir: VfsPath,
    pub project_dir: Option<VfsPath>,
}

impl Config {
    #[instrument(skip_all)]
    pub fn rubies(&self) -> Result<Vec<Ruby>> {
        let mut rubies = Vec::new();

        for ruby_dir in &self.ruby_dirs {
            if !ruby_dir.exists().unwrap_or(false) {
                continue;
            }

            if let Ok(entries) = ruby_dir.read_dir() {
                for entry in entries {
                    if let Ok(metadata) = entry.metadata()
                        && metadata.file_type == vfs::VfsFileType::Directory
                        && let Ok(ruby) = Ruby::from_dir(entry.as_str().into())
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

    pub fn get_project_dir(&self) -> Result<&VfsPath, Error> {
        match self.project_dir {
            None => Err(Error::NoProjectDir {
                current_dir: self.current_dir.clone(),
            }),
            Some(ref dir) => Ok(dir),
        }
    }
}

/// Default Ruby installation directories
pub fn default_ruby_dirs(root: &VfsPath) -> Vec<VfsPath> {
    vec![
        shellexpand::tilde("~/.rubies").as_ref(),
        "/opt/rubies",
        "/usr/local/rubies",
    ]
    .into_iter()
    .filter_map(|path| root.join(path).ok())
    .collect()
}
