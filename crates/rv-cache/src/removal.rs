use std::fmt::Display;
use std::io;
use std::ops::{Add, AddAssign};

use camino::{Utf8Path, Utf8PathBuf};
use tracing::debug;

use crate::CleanReporter;

pub struct Remover {
    reporter: Box<dyn CleanReporter>,
}

impl Remover {
    pub fn new(reporter: Box<dyn CleanReporter>) -> Self {
        Self { reporter }
    }

    pub fn rm_rf(&self, path: &Utf8Path) -> Result<Removal, io::Error> {
        debug!("Removing cache entry: {}", path);

        if !path.exists() {
            return Ok(Removal::default());
        }

        let removal = if path.is_dir() {
            let removal = self.rm_rf_dir(path)?;
            fs_err::remove_dir(path)?;
            self.reporter.on_clean();
            removal + Removal::new(1, 0)
        } else {
            let metadata = fs_err::metadata(path)?;
            fs_err::remove_file(path)?;
            self.reporter.on_clean();
            Removal::new(0, metadata.len())
        };

        Ok(removal)
    }

    fn rm_rf_dir(&self, path: &Utf8Path) -> Result<Removal, io::Error> {
        let mut removal = Removal::default();

        for entry in fs_err::read_dir(path)? {
            let entry = entry?;
            let entry_path = Utf8PathBuf::try_from(entry.path())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 path"))?;

            if entry_path.is_dir() {
                removal += self.rm_rf_dir(&entry_path)?;
                fs_err::remove_dir(&entry_path)?;
                self.reporter.on_clean();
                removal += Removal::new(1, 0);
            } else {
                let metadata = entry.metadata()?;
                fs_err::remove_file(&entry_path)?;
                self.reporter.on_clean();
                removal += Removal::new(0, metadata.len());
            }
        }

        Ok(removal)
    }
}

/// Remove a file or directory recursively.
pub fn rm_rf(path: impl AsRef<Utf8Path>) -> Result<Removal, io::Error> {
    let path = path.as_ref();

    if !path.exists() {
        return Ok(Removal::default());
    }

    if path.is_dir() {
        let mut removal = Removal::default();

        for entry in fs_err::read_dir(path)? {
            let entry = entry?;
            let entry_path = Utf8PathBuf::try_from(entry.path())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 path"))?;
            removal += rm_rf(&entry_path)?;
        }

        fs_err::remove_dir(path)?;
        removal += Removal::new(1, 0);

        Ok(removal)
    } else {
        let metadata = fs_err::metadata(path)?;
        fs_err::remove_file(path)?;
        Ok(Removal::new(0, metadata.len()))
    }
}

/// A summary of the files and directories removed from the cache.
#[derive(Debug, Default, Clone)]
pub struct Removal {
    /// The number of directories removed.
    pub dirs: u64,
    /// The number of bytes removed.
    pub bytes: u64,
}

impl Removal {
    pub fn new(dirs: u64, bytes: u64) -> Self {
        Self { dirs, bytes }
    }

    /// Returns `true` if no files or directories were removed.
    pub fn is_empty(&self) -> bool {
        self.dirs == 0 && self.bytes == 0
    }
}

impl Add for Removal {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            dirs: self.dirs + other.dirs,
            bytes: self.bytes + other.bytes,
        }
    }
}

impl AddAssign for Removal {
    fn add_assign(&mut self, other: Self) {
        self.dirs += other.dirs;
        self.bytes += other.bytes;
    }
}

impl Display for Removal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.dirs == 0 && self.bytes == 0 {
            write!(f, "No cache entries removed")
        } else if self.dirs == 0 {
            write!(f, "Removed {} bytes", self.bytes)
        } else if self.bytes == 0 {
            write!(f, "Removed {} directories", self.dirs)
        } else {
            write!(
                f,
                "Removed {} directories ({} bytes)",
                self.dirs, self.bytes
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use std::fs;

    struct TestReporter {
        clean_count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl TestReporter {
        fn new() -> (Self, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
            let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            (
                Self {
                    clean_count: counter.clone(),
                },
                counter,
            )
        }
    }

    impl CleanReporter for TestReporter {
        fn on_clean(&self) {
            self.clean_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        fn on_complete(&self) {
            // Test implementation - nothing special needed
        }
    }

    #[test]
    fn test_remover_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let (reporter, counter) = TestReporter::new();
        let remover = Remover::new(Box::new(reporter));

        let nonexistent_path =
            Utf8PathBuf::from_path_buf(temp_dir.path().join("nonexistent")).unwrap();
        let result = remover.rm_rf(&nonexistent_path).unwrap();

        assert!(result.is_empty());
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_remover_single_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("test.txt")).unwrap();

        // Create test file
        fs::write(&file_path, "test content").unwrap();
        let file_size = fs::metadata(&file_path).unwrap().len();

        let (reporter, counter) = TestReporter::new();
        let remover = Remover::new(Box::new(reporter));

        let result = remover.rm_rf(&file_path).unwrap();

        assert_eq!(result.dirs, 0);
        assert_eq!(result.bytes, file_size);
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(!file_path.exists());
    }

    #[test]
    fn test_remover_directory_with_contents() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("test_dir")).unwrap();
        let file_path = dir_path.join("test.txt");
        let subdir_path = dir_path.join("subdir");
        let subfile_path = subdir_path.join("sub.txt");

        // Create directory structure
        fs::create_dir_all(&subdir_path).unwrap();
        fs::write(&file_path, "test content").unwrap();
        fs::write(&subfile_path, "sub content").unwrap();

        let (reporter, counter) = TestReporter::new();
        let remover = Remover::new(Box::new(reporter));

        let result = remover.rm_rf(&dir_path).unwrap();

        // Should report: 2 files + 2 directories (subdir + main dir)
        assert_eq!(result.dirs, 2);
        assert!(result.bytes > 0); // Should have removed some bytes
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 4); // 2 files + 2 dirs
        assert!(!dir_path.exists());
    }

    #[test]
    fn test_removal_arithmetic() {
        let removal1 = Removal::new(2, 100);
        let removal2 = Removal::new(3, 200);

        let sum = removal1.clone() + removal2.clone();
        assert_eq!(sum.dirs, 5);
        assert_eq!(sum.bytes, 300);

        let mut addassign_test = removal1;
        addassign_test += removal2;
        assert_eq!(addassign_test.dirs, 5);
        assert_eq!(addassign_test.bytes, 300);
    }

    #[test]
    fn test_removal_is_empty() {
        let empty_removal = Removal::default();
        assert!(empty_removal.is_empty());

        let non_empty_removal = Removal::new(1, 0);
        assert!(!non_empty_removal.is_empty());

        let non_empty_bytes = Removal::new(0, 100);
        assert!(!non_empty_bytes.is_empty());
    }

    #[test]
    fn test_removal_display() {
        let empty = Removal::default();
        assert_eq!(empty.to_string(), "No cache entries removed");

        let bytes_only = Removal::new(0, 500);
        assert_eq!(bytes_only.to_string(), "Removed 500 bytes");

        let dirs_only = Removal::new(3, 0);
        assert_eq!(dirs_only.to_string(), "Removed 3 directories");

        let both = Removal::new(2, 1024);
        assert_eq!(both.to_string(), "Removed 2 directories (1024 bytes)");
    }
}
