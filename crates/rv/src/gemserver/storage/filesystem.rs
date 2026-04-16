use super::{Blob, Result, Storage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Internal struct for JSON serialization of blob metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetadataFile {
    bytesize: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
}

/// Filesystem-based storage implementation
///
/// Stores blobs as files on the filesystem with metadata stored in separate files.
/// For each blob, creates:
/// - Main file: Contains the content
/// - `.etag` file: Contains the ETag if present
///
/// # Example
///
/// ```rust
/// use crate::gemserver::storage::{FilesystemStorage, Storage, Blob};
/// use std::path::PathBuf;
/// use tempfile::TempDir;
///
/// #[tokio::main]
/// async fn main() {
///     let temp_dir = TempDir::new().unwrap();
///     let storage = FilesystemStorage::new(temp_dir.path().to_path_buf());
///
///     let blob = Blob::new(b"content".to_vec())
///         .with_etag("etag123".to_string());
///     storage.write_blob("test", &blob).await.unwrap();
///
///     let read_blob = storage.read_blob("test").await.unwrap();
///     assert_eq!(read_blob.content, b"content");
///     assert_eq!(read_blob.etag(), Some("etag123"));
/// }
/// ```
pub struct FilesystemStorage {
    base_path: PathBuf,
}

impl FilesystemStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn info_path(&self, key: &str) -> PathBuf {
        self.base_path.join("info").join(key)
    }

    fn metadata_path(&self, key: &str) -> PathBuf {
        self.base_path
            .join("metadata")
            .join(format!("{}.json", key))
    }
}

impl FilesystemStorage {
    // Private helper methods for implementation
    async fn read(&self, key: &str) -> Result<Vec<u8>> {
        Ok(tokio::fs::read(self.info_path(key)).await?)
    }

    async fn write(&self, key: &str, content: &[u8]) -> Result<()> {
        let path = self.info_path(key);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        Ok(tokio::fs::write(&path, content).await?)
    }

    async fn write_metadata_file(&self, key: &str, metadata: &MetadataFile) -> Result<()> {
        let path = self.metadata_path(key);

        // Create metadata directory if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(metadata)?;

        Ok(tokio::fs::write(&path, json).await?)
    }

    async fn read_metadata_file(&self, key: &str) -> Option<MetadataFile> {
        let path = self.metadata_path(key);

        // Try reading metadata JSON directly - no need to check existence first
        // This eliminates one async fs::metadata() call per read
        if let Ok(json) = tokio::fs::read_to_string(&path).await
            && let Ok(metadata) = serde_json::from_str(&json)
        {
            return Some(metadata);
        }

        None
    }
}

#[async_trait]
impl Storage for FilesystemStorage {
    async fn exists(&self, key: &str) -> bool {
        tokio::fs::metadata(self.info_path(key)).await.is_ok()
    }

    async fn read_blob(&self, key: &str) -> Result<Blob> {
        let content = self.read(key).await?;
        let metadata_file = self.read_metadata_file(key).await;

        Ok(Blob {
            content,
            etag: metadata_file.as_ref().and_then(|m| m.etag.clone()),
            sha256: metadata_file.as_ref().and_then(|m| m.sha256.clone()),
        })
    }

    async fn write_blob(&self, key: &str, blob: &Blob) -> Result<()> {
        self.write(key, &blob.content).await?;

        let metadata = MetadataFile {
            bytesize: blob.size(),
            etag: blob.etag.clone(),
            sha256: blob.sha256.clone(),
        };

        self.write_metadata_file(key, &metadata).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_filesystem_storage_exists() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path().to_path_buf());

        // File doesn't exist yet
        assert!(!storage.exists("foo").await);

        // Create file
        let info_root = temp_dir.path().join("info");
        tokio::fs::create_dir_all(&info_root).await.unwrap();
        fs::write(info_root.join("foo"), b"content").unwrap();

        // Now it exists
        assert!(storage.exists("foo").await);
    }

    #[tokio::test]
    async fn test_filesystem_storage_read_blob() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path().to_path_buf());

        let content = b"test content";
        let info_root = temp_dir.path().join("info");
        tokio::fs::create_dir_all(&info_root).await.unwrap();
        fs::write(info_root.join("foo"), content).unwrap();

        let blob = storage.read_blob("foo").await.unwrap();
        assert_eq!(blob.content, content);
        assert_eq!(blob.etag(), None);
    }

    #[tokio::test]
    async fn test_filesystem_storage_read_blob_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path().to_path_buf());

        let result = storage.read_blob("nonexistent");
        assert!(result.await.is_err());
    }

    #[tokio::test]
    async fn test_filesystem_storage_write_blob() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path().to_path_buf());

        let blob = Blob::new(b"test content".to_vec());
        storage.write_blob("foo", &blob).await.unwrap();

        let read_content = fs::read(temp_dir.path().join("info/foo")).unwrap();
        assert_eq!(read_content, b"test content");
    }

    #[tokio::test]
    async fn test_filesystem_storage_blob_operations() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path().to_path_buf());

        let blob = Blob::new(b"test content".to_vec()).with_etag("test-etag".to_string());
        storage.write_blob("testfile", &blob).await.unwrap();

        let read_blob = storage.read_blob("testfile").await.unwrap();
        assert_eq!(read_blob.content, blob.content);
        assert_eq!(read_blob.etag(), Some("test-etag"));
    }

    #[tokio::test]
    async fn test_filesystem_storage_blob_without_etag() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FilesystemStorage::new(temp_dir.path().to_path_buf());

        // Write blob without etag
        let blob = Blob::new(b"content".to_vec());
        storage.write_blob("testfile", &blob).await.unwrap();

        // Read it back - should have no etag
        let read_blob = storage.read_blob("testfile").await.unwrap();
        assert_eq!(read_blob.content, b"content");
        assert_eq!(read_blob.etag, None);
    }
}
