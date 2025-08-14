use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hasher;
use std::path::Path;
use std::time::SystemTime;

use crate::CacheKey;

/// A timestamp used to measure changes to a file.
///
/// On Unix, this uses `ctime` as a conservative approach. `ctime` should detect all
/// modifications, including some that we don't care about, like hardlink modifications.
/// On other platforms, it uses `mtime`.
///
/// See: <https://github.com/restic/restic/issues/2179>
/// See: <https://apenwarr.ca/log/20181113>
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Timestamp(SystemTime);

impl Timestamp {
    /// Return the [`Timestamp`] for the given path.
    pub fn from_path(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let metadata = fs_err::metadata(path.as_ref())?;
        Ok(Self::from_metadata(&metadata))
    }

    /// Return the [`Timestamp`] for the given metadata.
    pub fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            // Use ctime on Unix for more reliable change detection
            let ctime = u64::try_from(metadata.ctime()).unwrap_or(0);
            let ctime_nsec = u32::try_from(metadata.ctime_nsec()).unwrap_or(0);
            let duration = std::time::Duration::new(ctime, ctime_nsec);
            Self(std::time::UNIX_EPOCH + duration)
        }

        #[cfg(not(unix))]
        {
            // Fall back to mtime on other platforms
            let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
            Self(modified)
        }
    }

    /// Return the current [`Timestamp`].
    pub fn now() -> Self {
        Self(SystemTime::now())
    }

    /// Return the underlying [`SystemTime`].
    pub fn system_time(&self) -> SystemTime {
        self.0
    }
}

impl From<SystemTime> for Timestamp {
    fn from(system_time: SystemTime) -> Self {
        Self(system_time)
    }
}

impl From<Timestamp> for SystemTime {
    fn from(timestamp: Timestamp) -> Self {
        timestamp.0
    }
}

impl CacheKey for Timestamp {
    fn cache_key(&self, state: &mut crate::CacheKeyHasher) {
        state.write(format!("{:?}", self.0).as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_timestamp_ordering() {
        let t1 = Timestamp::now();
        thread::sleep(Duration::from_millis(10));
        let t2 = Timestamp::now();

        assert!(t2 > t1);
    }

    #[test]
    fn test_timestamp_from_system_time() {
        let system_time = SystemTime::now();
        let timestamp = Timestamp::from(system_time);
        let converted_back: SystemTime = timestamp.into();

        assert_eq!(system_time, converted_back);
    }

    #[test]
    fn test_timestamp_from_path() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs_err::write(&file_path, "test content").unwrap();

        let timestamp = Timestamp::from_path(&file_path).unwrap();

        // Should be recent
        let now = Timestamp::now();
        assert!(timestamp <= now);

        // But not too old (within last minute)
        let minute_ago = Timestamp::from(now.system_time() - Duration::from_secs(60));
        assert!(timestamp > minute_ago);
    }

    #[test]
    fn test_timestamp_serialization() {
        let timestamp = Timestamp::now();

        // Test JSON serialization
        let json = serde_json::to_string(&timestamp).unwrap();
        let deserialized: Timestamp = serde_json::from_str(&json).unwrap();

        assert_eq!(timestamp, deserialized);
    }
}
