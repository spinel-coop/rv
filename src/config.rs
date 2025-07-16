use miette::Result;
use std::path::PathBuf;
use tracing::instrument;
use vfs::VfsPath;

use crate::ruby::Ruby;

#[derive(Debug)]
pub struct Config {
    pub ruby_dirs: Vec<VfsPath>,
    pub gemfile: Option<PathBuf>,
    pub cache_dir: PathBuf,
    pub local_dir: PathBuf,
    pub root: VfsPath,
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
                        && let Ok(ruby) = Ruby::from_dir(entry)
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
