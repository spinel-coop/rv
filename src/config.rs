use miette::Result;
use std::path::PathBuf;

use crate::env::Ruby;

#[derive(Debug, Clone)]
pub struct Config {
    pub ruby_dirs: Vec<PathBuf>,

    pub gemfile: Option<PathBuf>,

    pub cache_dir: PathBuf,
    pub local_dir: PathBuf,
}

impl Config {
    pub fn rubies(&self) -> Result<Vec<Ruby>> {
        Ok(self
            .ruby_dirs
            .iter()
            .flat_map(|_dir| vec![Ruby {}])
            .collect())
    }
}
