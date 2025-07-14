use miette::Result;
use std::path::PathBuf;

use crate::ruby::Ruby;

#[derive(Debug, Clone)]
pub struct Config {
    pub ruby_dirs: Vec<PathBuf>,
    pub gemfile: Option<PathBuf>,
    pub cache_dir: PathBuf,
    pub local_dir: PathBuf,
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
        }
    }
    
    pub fn rubies(&self) -> Result<Vec<Ruby>> {
        discover_rubies(self)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

/// Default Ruby installation directories
pub fn default_ruby_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from(shellexpand::tilde("~/.rubies").as_ref()),
        PathBuf::from("/opt/rubies"),
        PathBuf::from("/usr/local/rubies"),
    ]
}

/// Discover Ruby installations from configured directories
pub fn discover_rubies(config: &Config) -> Result<Vec<Ruby>> {
    let mut rubies = Vec::new();
    
    for ruby_dir in &config.ruby_dirs {
        if !ruby_dir.exists() {
            continue;
        }
        
        if let Ok(entries) = std::fs::read_dir(ruby_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                    if let Ok(ruby) = Ruby::from_dir(entry.path()) {
                        if ruby.is_valid() {
                            rubies.push(ruby);
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
