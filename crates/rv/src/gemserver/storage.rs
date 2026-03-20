mod filesystem;

pub use filesystem::FilesystemStorage;

use async_trait::async_trait;
use base64::Engine;
use sha2::Digest;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error("Cannot verify empty content against digest")]
    EmptyContent,
    #[error(
        "The checksum of /{path} does not match the checksum provided by the server! Something is wrong. {message}"
    )]
    MismatchedChecksum { path: String, message: String },
}

pub type Result<T> = std::result::Result<T, Error>;

/// Represents a blob with content and optional metadata
///
/// A blob is the fundamental unit of storage in this library. It combines
/// raw content (bytes) with optional metadata for cache validation and
/// integrity checking.
///
/// # Fields
///
/// - `content`: The raw bytes of the stored data
/// - `etag`: Optional ETag for HTTP cache validation (RFC 7232)
/// - `sha256`: Optional SHA256 checksum in base64 format
///
/// # Checksums
///
/// Checksums can be provided when creating a blob (e.g., from HTTP headers
/// or parsed metadata), or calculated on-demand
///
/// # Example
///
/// ```rust
/// use crate::gemserver::storage::Blob;
///
/// // Create a blob without metadata
/// let blob = Blob::new(b"Hello, World!".to_vec());
/// assert_eq!(blob.etag, None);
///
/// // Create a blob with metadata
/// let blob = Blob::new(b"content".to_vec())
///     .with_etag("W/\"abc123\"".to_string());
/// assert_eq!(blob.etag, Some("W/\"abc123\"".to_string()));
/// ```
#[derive(Debug, Clone)]
pub struct Blob {
    pub content: Vec<u8>,
    pub etag: Option<String>,
    pub sha256: Option<String>,
}

impl Blob {
    pub fn new(content: Vec<u8>) -> Self {
        Self {
            content,
            etag: None,
            sha256: None,
        }
    }

    pub fn with_etag(mut self, etag: String) -> Self {
        self.etag = Some(etag);
        self
    }

    pub fn with_sha256(mut self, sha256: String) -> Self {
        self.sha256 = Some(sha256);
        self
    }

    /// Create a new blob by appending data to this blob's content with new metadata.
    ///
    /// Returns a new `Blob` with the combined content and the provided metadata.
    ///
    /// # Example
    ///
    /// ```rust
    /// use crate::gemserver::storage::Blob;
    ///
    /// let original = Blob::new(b"Hello, ".to_vec());
    ///
    /// let updated = original.append(b"World!", Some("new-etag".to_string()), None);
    ///
    /// assert_eq!(updated.content, b"Hello, World!");
    /// assert_eq!(updated.etag, Some("new-etag".to_string()));
    /// ```
    pub fn append(mut self, data: &[u8], etag: Option<String>, sha256: Option<String>) -> Self {
        self.content.extend_from_slice(data);
        Self {
            content: self.content,
            etag,
            sha256,
        }
    }

    /// Create a new blob with content and optional metadata.
    pub fn with_metadata(content: Vec<u8>, etag: Option<String>, sha256: Option<String>) -> Self {
        Self {
            content,
            etag,
            sha256,
        }
    }

    pub fn size(&self) -> u64 {
        self.content.len() as u64
    }

    /// Get ETag if available
    pub fn etag(&self) -> Option<&str> {
        self.etag.as_deref()
    }

    /// Get SHA256 checksum if available
    pub fn sha256(&self) -> Option<&str> {
        self.sha256.as_deref()
    }

    /// Verify the blob's content matches its stored sha256 digest.
    ///
    /// Returns Ok(()) if no sha256 is set or if the digest matches.
    /// Returns Err if the content doesn't match the stored digest.
    pub fn verify(&self) -> Result<()> {
        if let Some(expected) = &self.sha256 {
            if self.content.is_empty() {
                return Err(Error::EmptyContent);
            }

            let hash = sha2::Sha256::digest(&self.content);
            let actual = base64::engine::general_purpose::STANDARD.encode(hash);

            if &actual != expected {
                return Err(Error::MismatchedChecksum {
                    path: "blob".to_string(),
                    message: format!("Expected digest {}, got {}", expected, actual),
                });
            }
        }
        Ok(())
    }
}

/// Trait for abstract storage backends (filesystem, S3, etc.)
///
/// This trait defines the interface for storage backends used by the gem index client.
/// It provides a simple abstraction over different storage mechanisms, allowing the
/// library to work with filesystems, cloud storage, key-value stores, or any other
/// storage backend.
///
/// # Required Methods
///
/// Implementations must provide three methods:
/// - `exists`: Check if a blob exists at the given key
/// - `read_blob`: Read a blob and its metadata from storage
/// - `write_blob`: Write a blob and its metadata to storage
///
/// # The Blob Type
///
/// All operations work with [`Blob`] objects, which contain:
/// - `content`: The raw bytes of the stored data
/// - `etag`: Optional ETag metadata for cache validation
/// - `sha256`: Optional SHA256 checksum
///
/// # Implementing Custom Storage Backends
///
/// Each storage backend can handle metadata differently based on its
/// capabilities. See the [`FilesystemStorage`] for an example.
///
#[async_trait]
pub trait Storage: Send + Sync {
    /// Check if a file exists in storage
    async fn exists(&self, key: &str) -> bool;

    /// Read a blob with its metadata from storage
    async fn read_blob(&self, key: &str) -> Result<Blob>;

    /// Write a blob with its metadata to storage
    async fn write_blob(&self, key: &str, blob: &Blob) -> Result<()>;
}
