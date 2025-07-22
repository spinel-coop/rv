use crate::Result;
use rv_gem_types::Specification;
use std::collections::HashMap;
use std::path::Path;

pub struct Package<S> {
    source: S,
    spec: Option<Specification>,
    checksums: Option<HashMap<String, HashMap<String, String>>>,
}

impl Package<std::fs::File> {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        Ok(Self {
            source: file,
            spec: None,
            checksums: None,
        })
    }
}

impl<S> Package<S> {
    pub fn spec(&mut self) -> Result<&Specification> {
        todo!("implement in phase 3")
    }

    pub fn verify(&mut self) -> Result<()> {
        todo!("implement in phase 5")
    }
}
