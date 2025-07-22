use crate::{Error, Result};
use std::io::Read;
use tar::{Archive, Header};

/// Represents a file entry within a gem
#[derive(Debug, Clone)]
pub struct Entry {
    /// Path of the file within the gem
    pub path: String,
    /// Size of the file in bytes
    pub size: u64,
    /// File mode (permissions)
    pub mode: u32,
    /// Type of the entry
    pub entry_type: EntryType,
}

/// Type of entry in the gem
#[derive(Debug, Clone, PartialEq)]
pub enum EntryType {
    /// Regular file
    File,
    /// Directory
    Directory,
    /// Symbolic link
    Symlink { target: String },
}

impl Entry {
    pub fn new(path: String, size: u64, mode: u32, entry_type: EntryType) -> Self {
        Self {
            path,
            size,
            mode,
            entry_type,
        }
    }

    /// Create an Entry from a tar header
    pub fn from_tar_header(header: &Header, path: String) -> Result<Self> {
        let size = header.size()?;
        let mode = header.mode()?;

        let entry_type = match header.entry_type() {
            tar::EntryType::Regular => EntryType::File,
            tar::EntryType::Directory => EntryType::Directory,
            tar::EntryType::Symlink | tar::EntryType::Link => {
                let target = header
                    .link_name()
                    .map_err(|e| Error::tar_error(e.to_string()))?
                    .ok_or_else(|| Error::tar_error("Symlink missing target".to_string()))?
                    .to_string_lossy()
                    .to_string();
                EntryType::Symlink { target }
            }
            _ => {
                return Err(Error::tar_error(format!(
                    "Unsupported entry type: {:?}",
                    header.entry_type()
                )))
            }
        };

        Ok(Self {
            path,
            size,
            mode,
            entry_type,
        })
    }

    /// Check if this entry is a regular file
    pub fn is_file(&self) -> bool {
        matches!(self.entry_type, EntryType::File)
    }

    /// Check if this entry is a directory
    pub fn is_directory(&self) -> bool {
        matches!(self.entry_type, EntryType::Directory)
    }

    /// Check if this entry is a symbolic link
    pub fn is_symlink(&self) -> bool {
        matches!(self.entry_type, EntryType::Symlink { .. })
    }

    /// Get the symlink target if this is a symlink
    pub fn symlink_target(&self) -> Option<&str> {
        match &self.entry_type {
            EntryType::Symlink { target } => Some(target),
            _ => None,
        }
    }
}

/// Iterator over gem data entries with streaming content access
pub struct DataReader<R: Read> {
    archive: Archive<R>,
}

impl<R: Read> DataReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            archive: Archive::new(reader),
        }
    }

    /// Find and read a specific file by path
    pub fn find_file(&mut self, target_path: &str) -> Result<Option<Vec<u8>>> {
        for entry_result in self.archive.entries()? {
            let mut entry = entry_result.map_err(|e| Error::tar_error(e.to_string()))?;
            let path = entry
                .header()
                .path()
                .map_err(|e| Error::tar_error(e.to_string()))?;
            let path_str = path.to_string_lossy();

            if path_str == target_path {
                let mut content = Vec::new();
                entry.read_to_end(&mut content)?;
                return Ok(Some(content));
            }
        }
        Ok(None)
    }

    /// Convert to a vector of all entries (for convenience)
    pub fn collect_entries(&mut self) -> Result<Vec<Entry>> {
        let mut result = Vec::new();

        for entry_result in self.archive.entries()? {
            let entry = entry_result.map_err(|e| Error::tar_error(e.to_string()))?;
            let header = entry.header();
            let path = header
                .path()
                .map_err(|e| Error::tar_error(e.to_string()))?
                .to_string_lossy()
                .to_string();

            result.push(Entry::from_tar_header(header, path)?);
        }

        Ok(result)
    }
}
