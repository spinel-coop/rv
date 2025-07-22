use crate::Result;
use std::io::{Read, Seek};

/// Trait for sources that can provide .gem file data
pub trait PackageSource: Read + Seek {
    /// Check if the source is seekable (for optimization)
    fn is_seekable(&self) -> bool {
        true
    }

    /// Get the total size of the source if known
    fn size(&self) -> Result<Option<u64>>;
}

impl PackageSource for std::fs::File {
    fn size(&self) -> Result<Option<u64>> {
        Ok(Some(self.metadata()?.len()))
    }
}

impl<T: AsRef<[u8]>> PackageSource for std::io::Cursor<T> {
    fn size(&self) -> Result<Option<u64>> {
        Ok(Some(self.get_ref().as_ref().len() as u64))
    }
}
