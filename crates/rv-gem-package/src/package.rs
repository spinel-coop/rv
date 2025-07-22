use crate::{checksum::Checksums, entry::DataReader, source::PackageSource, Result};
use rv_gem_types::Specification;
use std::path::Path;

/// A .gem package that can be read and analyzed
pub struct Package<S: PackageSource> {
    source: S,
    spec: Option<Specification>,
    checksums: Option<Checksums>,
}

impl Package<std::fs::File> {
    /// Open a .gem file from the filesystem
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        Ok(Self {
            source: file,
            spec: None,
            checksums: None,
        })
    }
}

impl<S: PackageSource> Package<S> {
    /// Create a new package from any source
    pub fn from_source(source: S) -> Self {
        Self {
            source,
            spec: None,
            checksums: None,
        }
    }

    /// Get the gem specification (lazy loaded)
    pub fn spec(&mut self) -> Result<&Specification> {
        todo!("implement in phase 3")
    }

    /// Get access to the data.tar.gz contents for streaming
    pub fn data(&mut self) -> Result<DataReader<Box<dyn std::io::Read>>> {
        todo!("implement in phase 4")
    }

    /// Verify the package checksums
    pub fn verify(&mut self) -> Result<()> {
        todo!("implement in phase 5")
    }

    /// Get the checksums (lazy loaded)
    pub fn checksums(&mut self) -> Result<&Checksums> {
        todo!("implement in phase 3")
    }
}
