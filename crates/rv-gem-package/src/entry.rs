use std::io::Read;

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
}

/// Iterator over gem data entries with streaming content access
pub struct DataReader<R> {
    reader: R,
}

impl<R: Read> DataReader<R> {
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}

impl<R: Read> Iterator for DataReader<R> {
    type Item = crate::Result<(Entry, Box<dyn Read>)>;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO: implement in phase 4
        None
    }
}
