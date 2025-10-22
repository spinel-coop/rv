use std::fmt::{Display, Formatter};
use std::io;
use std::io::Write;
use std::ops::Deref;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use tracing::debug;

#[cfg(feature = "clap")]
pub use crate::cli::CacheArgs;
use crate::removal::Remover;
pub use crate::removal::{Removal, rm_rf};

// Re-export our custom caching utilities
pub use crate::cache_key::{CacheKey, CacheKeyHasher, cache_digest};
pub use crate::timestamp::Timestamp;

mod cache_key;
#[cfg(feature = "clap")]
mod cli;
mod removal;
mod timestamp;

/// A [`CacheEntry`] which may or may not exist yet.
#[derive(Debug, Clone)]
pub struct CacheEntry(Utf8PathBuf);

impl CacheEntry {
    /// Create a new [`CacheEntry`] from a directory and a file name.
    pub fn new(dir: impl Into<Utf8PathBuf>, file: impl AsRef<Utf8Path>) -> Self {
        Self(dir.into().join(file))
    }

    /// Create a new [`CacheEntry`] from a path.
    pub fn from_path(path: impl Into<Utf8PathBuf>) -> Self {
        Self(path.into())
    }

    /// Return the cache entry's parent directory.
    pub fn shard(&self) -> CacheShard {
        CacheShard(self.dir().to_path_buf())
    }

    /// Convert the [`CacheEntry`] into a [`Utf8PathBuf`].
    #[inline]
    pub fn into_path_buf(self) -> Utf8PathBuf {
        self.0
    }

    /// Return the path to the [`CacheEntry`].
    #[inline]
    pub fn path(&self) -> &Utf8Path {
        &self.0
    }

    /// Return the cache entry's parent directory.
    #[inline]
    pub fn dir(&self) -> &Utf8Path {
        self.0.parent().expect("Cache entry has no parent")
    }

    /// Create a new [`CacheEntry`] with the given file name.
    #[must_use]
    pub fn with_file(&self, file: impl AsRef<Utf8Path>) -> Self {
        Self(self.dir().join(file))
    }
}

impl AsRef<Utf8Path> for CacheEntry {
    fn as_ref(&self) -> &Utf8Path {
        &self.0
    }
}

/// A subdirectory within the cache.
#[derive(Debug, Clone)]
pub struct CacheShard(Utf8PathBuf);

impl CacheShard {
    /// Return a [`CacheEntry`] within this shard.
    pub fn entry(&self, file: impl AsRef<Utf8Path>) -> CacheEntry {
        CacheEntry::new(&self.0, file)
    }

    /// Return a [`CacheShard`] within this shard.
    #[must_use]
    pub fn shard(&self, dir: impl AsRef<Utf8Path>) -> Self {
        Self(self.0.join(dir.as_ref()))
    }

    /// Return the [`CacheShard`] as a [`Utf8PathBuf`].
    pub fn into_path_buf(self) -> Utf8PathBuf {
        self.0
    }
}

impl AsRef<Utf8Path> for CacheShard {
    fn as_ref(&self) -> &Utf8Path {
        &self.0
    }
}

impl Deref for CacheShard {
    type Target = Utf8Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The main cache abstraction.
#[derive(Debug, Clone)]
pub struct Cache {
    /// The cache directory.
    root: Utf8PathBuf,
    /// A temporary cache directory, if the user requested `--no-cache`.
    ///
    /// Included to ensure that the temporary directory exists for the length of the operation, but
    /// is dropped at the end as appropriate.
    temp_dir: Option<Arc<tempfile::TempDir>>,
}

impl Cache {
    /// A persistent cache directory at `root`.
    pub fn from_path(root: impl Into<Utf8PathBuf>) -> Self {
        Self {
            root: root.into(),
            temp_dir: None,
        }
    }

    /// Create a temporary cache directory.
    pub fn temp() -> Result<Self, io::Error> {
        let temp_dir = tempfile::tempdir()?;
        let root = Utf8PathBuf::try_from(temp_dir.path().to_path_buf())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 path"))?;
        Ok(Self {
            root,
            temp_dir: Some(Arc::new(temp_dir)),
        })
    }

    /// Return the root of the cache.
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// The folder for a specific cache bucket
    pub fn bucket(&self, cache_bucket: CacheBucket) -> Utf8PathBuf {
        self.root.join(cache_bucket.to_str())
    }

    /// Compute a shard in the cache.
    pub fn shard(&self, cache_bucket: CacheBucket, dir: impl AsRef<Utf8Path>) -> CacheShard {
        CacheShard(self.bucket(cache_bucket).join(dir.as_ref()))
    }

    /// Compute an entry in the cache.
    pub fn entry(
        &self,
        cache_bucket: CacheBucket,
        dir: impl AsRef<Utf8Path>,
        file: impl AsRef<Utf8Path>,
    ) -> CacheEntry {
        CacheEntry::new(self.bucket(cache_bucket).join(dir), file)
    }

    /// Returns `true` if the [`Cache`] is temporary.
    pub fn is_temporary(&self) -> bool {
        self.temp_dir.is_some()
    }

    /// Initialize the [`Cache`].
    pub fn init(self) -> Result<Self, io::Error> {
        let root = &self.root;

        // Create the cache directory, if it doesn't exist.
        fs_err::create_dir_all(root)?;

        // Add the .gitignore.
        match fs_err::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(".gitignore"))
        {
            Ok(mut file) => file.write_all(b"*")?,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => (),
            Err(err) => return Err(err),
        }

        Ok(Self {
            root: root.canonicalize_utf8().map_err(io::Error::other)?,
            ..self
        })
    }

    /// Clear the cache, removing all entries.
    pub fn clear(&self, reporter: Box<dyn CleanReporter>) -> Result<Removal, io::Error> {
        Remover::new(reporter).rm_rf(&self.root)
    }

    /// Run the garbage collector on the cache, removing any unused entries.
    pub fn prune(&self) -> Result<Removal, io::Error> {
        let mut summary = Removal::default();

        // Remove any top-level directories that are unused. These typically represent
        // outdated cache buckets (e.g., `ruby-v0`, when latest is `ruby-v0`).
        for entry in fs_err::read_dir(&self.root)? {
            let entry = entry?;
            let metadata = entry.metadata()?;

            if entry.file_name() == ".gitignore" {
                continue;
            }

            if metadata.is_dir() {
                // If the directory is not a cache bucket, remove it.
                let entry_name = entry.file_name();
                if CacheBucket::iter().all(|bucket| entry_name != bucket.to_str()) {
                    let path = Utf8PathBuf::try_from(entry.path()).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 path")
                    })?;
                    debug!("Removing dangling cache bucket: {}", path);
                    summary += rm_rf(path)?;
                }
            } else {
                // If the file is not a marker file, remove it.
                let path = Utf8PathBuf::try_from(entry.path()).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 path")
                })?;
                debug!("Removing dangling cache file: {}", path);
                summary += rm_rf(path)?;
            }
        }

        Ok(summary)
    }
}

pub trait CleanReporter: Send + Sync {
    /// Called after one file or directory is removed.
    fn on_clean(&self);

    /// Called after all files and directories are removed.
    fn on_complete(&self);
}

/// The different kinds of data in the cache are stored in different buckets, which in our case
/// are subdirectories of the cache root.
/// Cache structure: `<bucket>-v0/<digest(path)>.ext`
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum CacheBucket {
    /// Ruby interpreters
    Ruby,
    /// Gems
    Gem,
}

impl CacheBucket {
    fn to_str(self) -> &'static str {
        match self {
            Self::Ruby => "ruby-v0",
            Self::Gem => "gem-v0",
        }
    }

    /// Return an iterator over all cache buckets.
    pub fn iter() -> impl Iterator<Item = Self> {
        [Self::Ruby, Self::Gem].iter().copied()
    }
}

impl Display for CacheBucket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test reporter that tracks cleanup operations
    #[derive(Default, Debug)]
    struct TestReporter {
        cleaned: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        completed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl TestReporter {
        fn new() -> Self {
            Self::default()
        }

        fn cleaned_count(&self) -> usize {
            self.cleaned.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn is_completed(&self) -> bool {
            self.completed.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl CleanReporter for TestReporter {
        fn on_clean(&self) {
            self.cleaned
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        fn on_complete(&self) {
            self.completed
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[test]
    fn test_cache_bucket_display() {
        assert_eq!(CacheBucket::Ruby.to_string(), "ruby-v0");
    }

    #[test]
    fn test_cache_bucket_iteration() {
        let buckets: Vec<_> = CacheBucket::iter().collect();
        assert_eq!(buckets.len(), 2);
        assert!(buckets.contains(&CacheBucket::Ruby));
    }

    #[test]
    fn test_cache_entry_creation() {
        let entry = CacheEntry::new("/base/path", "file.json");
        assert_eq!(entry.path().as_str(), "/base/path/file.json");
        assert_eq!(entry.dir().as_str(), "/base/path");
    }

    #[test]
    fn test_cache_entry_with_file() {
        let entry = CacheEntry::new("/base/path", "file.json");
        let new_entry = entry.with_file("other.json");
        assert_eq!(new_entry.path().as_str(), "/base/path/other.json");
    }

    #[test]
    fn test_cache_entry_shard() {
        let entry = CacheEntry::new("/base/path/subdir", "file.json");
        let shard = entry.shard();
        assert_eq!(shard.as_ref().as_str(), "/base/path/subdir");
    }

    #[test]
    fn test_cache_shard_operations() {
        let shard = CacheShard(("/base/cache").into());

        let entry = shard.entry("file.json");
        assert_eq!(entry.path().as_str(), "/base/cache/file.json");

        let sub_shard = shard.shard("subdir");
        assert_eq!(sub_shard.as_ref().as_str(), "/base/cache/subdir");
    }

    #[test]
    fn test_cache_from_path() {
        let cache = Cache::from_path("/test/cache");
        assert_eq!(cache.root().as_str(), "/test/cache");
        assert!(!cache.is_temporary());
    }

    #[test]
    fn test_cache_temp() {
        let cache = Cache::temp().unwrap();
        assert!(cache.is_temporary());
        // Temp cache should have a valid root path
        assert!(!cache.root().as_str().is_empty());
    }

    #[test]
    fn test_cache_bucket_paths() {
        let cache = Cache::from_path("/test/cache");

        assert_eq!(
            cache.bucket(CacheBucket::Ruby).as_str(),
            "/test/cache/ruby-v0"
        );
    }

    #[test]
    fn test_cache_entry_operations() {
        let cache = Cache::from_path("/test/cache");

        let entry = cache.entry(CacheBucket::Ruby, "interpreters", "ruby-3.3.0.json");
        assert_eq!(
            entry.path().as_str(),
            "/test/cache/ruby-v0/interpreters/ruby-3.3.0.json"
        );
    }

    #[test]
    fn test_cache_initialization() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("cache");
        let cache_path_utf8 = camino::Utf8PathBuf::from(cache_path.to_str().unwrap());
        let _cache = Cache::from_path(&cache_path_utf8).init().unwrap();

        // Cache directory should be created
        assert!(cache_path.exists());
        assert!(cache_path.is_dir());

        // .gitignore should be created
        let gitignore_path = cache_path.join(".gitignore");
        assert!(gitignore_path.exists());

        // .gitignore should contain "*"
        let contents = fs_err::read_to_string(&gitignore_path).unwrap();
        assert_eq!(contents, "*");
    }

    #[test]
    fn test_cache_initialization_existing_gitignore() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("cache");
        let cache_path_utf8 = camino::Utf8PathBuf::from(cache_path.to_str().unwrap());
        fs_err::create_dir_all(&cache_path).unwrap();

        // Pre-create .gitignore
        let gitignore_path = cache_path.join(".gitignore");
        fs_err::write(&gitignore_path, "existing content").unwrap();

        let _cache = Cache::from_path(&cache_path_utf8).init().unwrap();

        // .gitignore content should be preserved
        let contents = fs_err::read_to_string(&gitignore_path).unwrap();
        assert_eq!(contents, "existing content");
    }

    #[test]
    fn test_cache_clear() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("cache");
        let cache_path_utf8 = camino::Utf8PathBuf::from(cache_path.to_str().unwrap());
        let cache = Cache::from_path(&cache_path_utf8).init().unwrap();

        // Create some test files
        let test_file = cache_path.join("test.txt");
        fs_err::write(&test_file, "test content").unwrap();

        let test_dir = cache_path.join("subdir");
        fs_err::create_dir(&test_dir).unwrap();
        fs_err::write(test_dir.join("file.txt"), "content").unwrap();

        let reporter = TestReporter::new();
        let removal = cache.clear(Box::new(reporter)).unwrap();

        assert!(!removal.is_empty());
        assert!(removal.bytes > 0);
    }

    #[test]
    fn test_cache_prune() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let cache_path = temp_dir.path().join("cache");
        let cache_path_utf8 = camino::Utf8PathBuf::from(cache_path.to_str().unwrap());
        let cache = Cache::from_path(&cache_path_utf8).init().unwrap();

        // Create a valid bucket directory
        let valid_bucket = cache_path.join("ruby-v0");
        fs_err::create_dir(&valid_bucket).unwrap();
        fs_err::write(valid_bucket.join("test.json"), "{}").unwrap();

        // Create an invalid bucket directory (old version)
        let invalid_bucket = cache_path.join("ruby-v-0");
        fs_err::create_dir(&invalid_bucket).unwrap();
        fs_err::write(invalid_bucket.join("old.json"), "{}").unwrap();

        // Create a random file (should be removed)
        fs_err::write(cache_path.join("random.txt"), "content").unwrap();

        let removal = cache.prune().unwrap();

        // Valid bucket should remain
        assert!(valid_bucket.exists());

        // Invalid bucket should be removed
        assert!(!invalid_bucket.exists());

        // Random file should be removed
        assert!(!cache_path.join("random.txt").exists());

        // .gitignore should remain
        assert!(cache_path.join(".gitignore").exists());

        assert!(!removal.is_empty());
    }

    #[test]
    fn test_removal_display() {
        let removal = super::removal::Removal::new(0, 0);
        assert_eq!(removal.to_string(), "No cache entries removed");

        let removal = super::removal::Removal::new(0, 1024);
        assert_eq!(removal.to_string(), "Removed 1024 bytes");

        let removal = super::removal::Removal::new(5, 0);
        assert_eq!(removal.to_string(), "Removed 5 directories");

        let removal = super::removal::Removal::new(3, 2048);
        assert_eq!(removal.to_string(), "Removed 3 directories (2048 bytes)");
    }

    #[test]
    fn test_removal_operations() {
        let mut removal1 = super::removal::Removal::new(2, 1000);
        let removal2 = super::removal::Removal::new(3, 2000);

        // Test Add trait
        let sum = removal1.clone() + removal2.clone();
        assert_eq!(sum.dirs, 5);
        assert_eq!(sum.bytes, 3000);

        // Test AddAssign trait
        removal1 += removal2;
        assert_eq!(removal1.dirs, 5);
        assert_eq!(removal1.bytes, 3000);

        // Test is_empty
        assert!(!removal1.is_empty());

        let empty = super::removal::Removal::new(0, 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_rm_rf_nonexistent_path() {
        let removal = super::removal::rm_rf("/nonexistent/path").unwrap();
        assert!(removal.is_empty());
    }

    #[test]
    fn test_rm_rf_file() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs_err::write(&file_path, "test content").unwrap();

        let utf8_path = camino::Utf8PathBuf::from(file_path.to_str().unwrap());
        let removal = super::removal::rm_rf(&utf8_path).unwrap();

        assert!(!file_path.exists());
        assert_eq!(removal.dirs, 0);
        assert!(removal.bytes > 0); // Should have removed some bytes
    }

    #[test]
    fn test_rm_rf_directory() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().join("test_dir");
        fs_err::create_dir(&dir_path).unwrap();

        // Create nested content
        fs_err::write(dir_path.join("file1.txt"), "content1").unwrap();

        let subdir = dir_path.join("subdir");
        fs_err::create_dir(&subdir).unwrap();
        fs_err::write(subdir.join("file2.txt"), "content2").unwrap();

        let utf8_path = camino::Utf8PathBuf::from(dir_path.to_str().unwrap());
        let removal = super::removal::rm_rf(&utf8_path).unwrap();

        assert!(!dir_path.exists());
        assert!(removal.dirs > 0); // Should have removed directories
        assert!(removal.bytes > 0); // Should have removed some bytes
    }

    #[test]
    fn test_test_reporter() {
        let reporter = TestReporter::new();

        assert_eq!(reporter.cleaned_count(), 0);
        assert!(!reporter.is_completed());

        reporter.on_clean();
        reporter.on_clean();
        assert_eq!(reporter.cleaned_count(), 2);

        reporter.on_complete();
        assert!(reporter.is_completed());
    }
}
