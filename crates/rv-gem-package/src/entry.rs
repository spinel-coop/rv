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
                    .link_name()?
                    .ok_or_else(Error::tar_missing_symlink_target)?
                    .to_string_lossy()
                    .to_string();
                EntryType::Symlink { target }
            }
            _ => {
                return Err(Error::tar_unsupported_entry_type(format!(
                    "{:?}",
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

/// Wrapper that provides streaming access to file contents
pub struct FileReader {
    content: std::io::Cursor<Vec<u8>>,
    metadata: Entry,
}

impl FileReader {
    pub fn new(content: Vec<u8>, metadata: Entry) -> Self {
        Self {
            content: std::io::Cursor::new(content),
            metadata,
        }
    }

    /// Get the file metadata
    pub fn metadata(&self) -> &Entry {
        &self.metadata
    }

    /// Get the file path
    pub fn path(&self) -> &str {
        &self.metadata.path
    }

    /// Get the file size
    pub fn size(&self) -> u64 {
        self.metadata.size
    }

    /// Check if this is a regular file
    pub fn is_file(&self) -> bool {
        self.metadata.is_file()
    }

    /// Get the content as bytes (already loaded)
    pub fn content(&self) -> &[u8] {
        self.content.get_ref()
    }
}

impl Read for FileReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.content.read(buf)
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

    /// Find a specific file by path and return a streaming reader
    pub fn find_file(&mut self, target_path: &str) -> Result<Option<FileReader>> {
        for entry_result in self.archive.entries()? {
            let mut entry = entry_result?;
            let path = entry
                .header()
                .path()
?;
            let path_str = path.to_string_lossy();

            if path_str == target_path {
                let metadata = Entry::from_tar_header(entry.header(), path_str.to_string())?;

                // Only load content for regular files
                if metadata.is_file() {
                    let mut content = Vec::new();
                    entry.read_to_end(&mut content)?;
                    return Ok(Some(FileReader::new(content, metadata)));
                } else {
                    // For directories and symlinks, return empty content
                    return Ok(Some(FileReader::new(Vec::new(), metadata)));
                }
            }
        }
        Ok(None)
    }

    /// Convert to a vector of all entries (for convenience)
    pub fn collect_entries(&mut self) -> Result<Vec<Entry>> {
        let mut result = Vec::new();

        for entry_result in self.archive.entries()? {
            let entry = entry_result?;
            let header = entry.header();
            let path = header
                .path()
?
                .to_string_lossy()
                .to_string();

            result.push(Entry::from_tar_header(header, path)?);
        }

        Ok(result)
    }
}
