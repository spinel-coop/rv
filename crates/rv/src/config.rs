use miette::Result;
use std::path::PathBuf;
use std::sync::Arc;
use vfs::{FileSystem, PhysicalFS, VfsPath};

use crate::ruby::Ruby;

const APP_PREFIX: &str = "rv";

#[derive(Debug)]
pub struct Config {
    pub ruby_dirs: Vec<VfsPath>,
    pub gemfile: Option<PathBuf>,
    pub cache_dir: PathBuf,
    pub local_dir: PathBuf,
    pub fs: Arc<dyn FileSystem + Send + Sync>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            ruby_dirs: default_ruby_dirs(),
            gemfile: None,
            cache_dir: xdg::BaseDirectories::with_prefix(APP_PREFIX)
                .cache_home
                .unwrap_or_else(|| std::env::temp_dir().join(APP_PREFIX)),
            local_dir: xdg::BaseDirectories::with_prefix(APP_PREFIX)
                .data_home
                .unwrap_or_else(|| std::env::temp_dir().join(APP_PREFIX)),
            fs: Arc::new(PhysicalFS::new("/")),
        }
    }

    pub fn rubies(&self) -> Result<Vec<Ruby>> {
        let mut rubies = Vec::new();

        for ruby_dir in &self.ruby_dirs {
            if !ruby_dir.exists().unwrap_or(false) {
                continue;
            }

            if let Ok(entries) = ruby_dir.read_dir() {
                for entry in entries {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.file_type == vfs::VfsFileType::Directory {
                            if let Ok(ruby) = Ruby::from_dir(entry) {
                                if ruby.is_valid() {
                                    rubies.push(ruby);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort rubies by implementation and version
        rubies.sort();

        Ok(rubies)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// Default Ruby installation directories
pub fn default_ruby_dirs() -> Vec<VfsPath> {
    let fs = PhysicalFS::new("/");
    let root = VfsPath::new(fs);

    vec![
        shellexpand::tilde("~/.rubies").as_ref(),
        "/opt/rubies",
        "/usr/local/rubies",
    ]
    .into_iter()
    .filter_map(|path| root.join(path).ok())
    .collect()
}
