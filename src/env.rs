use miette::{Result, bail};
use std::path::PathBuf;

pub struct Ruby {
    dir: PathBuf,
    engine: String,
    version: String,
    api_version: String,
    opt: String,
    gem_root: String,
    gem_path: String,
    gem_home: String,
}

impl Ruby {
    fn new(dir: PathBuf) -> Result<Self> {
        todo!()
    }
}

pub struct GemHome {}

pub struct Bundle {}
