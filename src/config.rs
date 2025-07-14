use miette::Result;
use std::path::PathBuf;
use std::sync::Arc;
use vfs::{FileSystem, PhysicalFS, VfsPath};

use crate::ruby::Ruby;

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
            cache_dir: xdg::BaseDirectories::with_prefix("rv")
                .cache_home
                .unwrap_or_else(|| std::env::temp_dir().join("rv")),
            local_dir: xdg::BaseDirectories::with_prefix("rv")
                .data_home
                .unwrap_or_else(|| std::env::temp_dir().join("rv")),
            fs: Arc::new(PhysicalFS::new("/")),
        }
    }

    pub fn rubies(&self) -> Result<Vec<Ruby>> {
        // Create a new PhysicalFS for this operation
        let fs = PhysicalFS::new("/");
        discover_rubies_vfs(self, fs)
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

/// Discover Ruby installations from configured directories using VFS
pub fn discover_rubies_vfs<T: FileSystem>(config: &Config, _fs: T) -> Result<Vec<Ruby>> {
    let mut rubies = Vec::new();

    for ruby_dir in &config.ruby_dirs {
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
