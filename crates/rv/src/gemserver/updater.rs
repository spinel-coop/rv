use std::collections::HashMap;
use std::sync::Arc;

use crate::gemserver::Error;
use crate::gemserver::http_fetcher::Fetcher;
use crate::gemserver::storage::Blob;

pub type Result<T> = std::result::Result<T, Error>;

pub struct Updater {
    fetcher: Arc<dyn Fetcher>,
}

impl Updater {
    pub fn new(fetcher: impl Fetcher + 'static) -> Self {
        Self {
            fetcher: Arc::new(fetcher),
        }
    }

    /// Update a file using an existing blob for incremental optimization
    ///
    /// If the blob is empty, automatically falls back to fetch().
    /// Otherwise, tries append first (using blob's etag for conditional request),
    /// falls back to fetch on failure.
    /// Returns the updated blob.
    pub async fn update(&self, remote_path: &str, blob: Blob) -> Result<Blob> {
        // Empty blob - go straight to fetch
        if blob.content.is_empty() {
            return self.fetch(remote_path).await;
        }

        let range_start = blob.size().saturating_sub(1) as usize;
        let etag = blob.etag();

        let response = self
            .fetcher
            .call(remote_path, Self::request_headers(etag, Some(range_start)))
            .await?;

        // Not modified - nothing to do
        if response.is_not_modified() {
            return Ok(blob);
        }

        // Empty or failed - fall back to fetch
        if response.body.is_empty() {
            return self.fetch(remote_path).await;
        }

        let new_etag = response.etag();
        let sha256 = response.digests().and_then(|d| d.get("sha-256").cloned());

        if response.is_partial_content() {
            // 206 Partial - append in memory and verify
            // Skip first byte (overlap with existing content)
            let new_blob = blob.append(&response.body[1..], new_etag, sha256);

            // Verify combined content with full SHA256
            if new_blob.verify().is_err() {
                // Verification failed - fall back to fetch
                return self.fetch(remote_path).await;
            }

            Ok(new_blob)
        } else {
            // 200 Full - server ignored range request and sent complete file
            // Use the response directly instead of making another request
            let new_blob = Blob::with_metadata(response.body, new_etag, sha256);
            new_blob.verify()?;
            Ok(new_blob)
        }
    }

    /// Fetch a fresh copy of the file (unconditional download)
    ///
    /// Use this when you don't have a cached blob or want a fresh download.
    /// Does not use etag/conditional requests - always downloads complete file.
    /// Returns the fetched blob.
    pub async fn fetch(&self, remote_path: &str) -> Result<Blob> {
        // Fetch the file without any conditional headers
        let response = self.fetcher.call(remote_path, HashMap::new()).await?;

        // Build the new blob with metadata
        let etag = response.etag();
        let sha256 = response.digests().and_then(|d| d.get("sha-256").cloned());

        let new_blob = Blob::with_metadata(response.body, etag, sha256);

        // Verify digest if provided (propagate errors since no fallback available)
        new_blob.verify()?;

        Ok(new_blob)
    }

    fn request_headers(etag: Option<&str>, range_start: Option<usize>) -> HashMap<String, String> {
        [
            range_start.map(|start| ("Range".to_string(), format!("bytes={}-", start))),
            etag.map(|tag| ("If-None-Match".to_string(), format!("\"{}\"", tag))),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gemserver::http_fetcher::MockFetcher;
    use crate::gemserver::storage::{self, Blob};
    use base64::Engine;
    use sha2::Digest;

    #[tokio::test]
    async fn test_when_local_path_does_not_exist_downloads_file_without_attempting_append() {
        let fetcher = MockFetcher::default();
        let full_body = b"abc123";

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"thisisanetag\"".to_string());

        fetcher.add_response(full_body.to_vec(), headers, 200);

        let updater = Updater::new(fetcher);
        let blob = updater.fetch("remote_path").await.unwrap();

        assert_eq!(blob.content, full_body);
        assert_eq!(blob.etag(), Some("thisisanetag"));
    }

    #[tokio::test]
    async fn test_when_local_path_does_not_exist_fails_immediately_on_bad_checksum() {
        let fetcher = MockFetcher::default();
        let full_body = b"abc123";

        let mut headers = HashMap::new();
        headers.insert("Repr-Digest".to_string(), "sha-256=:baddigest:".to_string());
        headers.insert("ETag".to_string(), "\"thisisanetag\"".to_string());

        fetcher.add_response(full_body.to_vec(), headers, 200);

        let updater = Updater::new(fetcher);
        let result = updater.fetch("remote_path").await;

        assert!(matches!(
            result,
            Err(Error::StorageError(
                storage::Error::MismatchedChecksum { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn test_when_local_path_exists_with_etag_does_nothing_if_etags_match() {
        let fetcher = MockFetcher::default();
        let local_body = b"abc";

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"LocalEtag\"".to_string());

        fetcher.add_response(vec![], headers, 304); // Not Modified

        let updater = Updater::new(fetcher);
        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        let result_blob = updater.update("remote_path", blob).await.unwrap();

        assert_eq!(result_blob.content, local_body);
        assert_eq!(result_blob.etag(), Some("LocalEtag"));
    }

    #[tokio::test]
    async fn test_when_local_path_exists_with_etag_appends_file_if_etags_do_not_match() {
        let fetcher = MockFetcher::default();
        let local_body = b"abc";
        let full_body = b"abc123";
        let hash = sha2::Sha256::digest(full_body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));
        headers.insert("ETag".to_string(), "\"NewEtag\"".to_string());

        fetcher.add_response(
            b"c123".to_vec(), // Partial content (skipping first 2 bytes)
            headers,
            206, // Partial Content
        );

        let updater = Updater::new(fetcher);
        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        let result_blob = updater.update("remote_path", blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("NewEtag"));
    }

    #[tokio::test]
    async fn test_when_local_path_exists_with_etag_replaces_file_if_response_ignores_range() {
        let fetcher = MockFetcher::default();
        let local_body = b"abc";
        let full_body = b"abc123";
        let hash = sha2::Sha256::digest(full_body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));
        headers.insert("ETag".to_string(), "\"NewEtag\"".to_string());

        fetcher.add_response(
            full_body.to_vec(),
            headers,
            200, // Full response, not partial
        );

        let updater = Updater::new(fetcher);
        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        let result_blob = updater.update("remote_path", blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("NewEtag"));
    }

    #[tokio::test]
    async fn test_tries_request_again_if_partial_response_fails_digest_check() {
        let fetcher = MockFetcher::default();
        let local_body = b"abc";
        let full_body = b"abc123";
        let hash = sha2::Sha256::digest(full_body);
        let good_digest = base64::engine::general_purpose::STANDARD.encode(hash);

        // First response: partial content with bad digest
        let mut headers1 = HashMap::new();
        headers1.insert("Repr-Digest".to_string(), "sha-256=:baddigest:".to_string());
        fetcher.add_response(b"the beginning of the file changed".to_vec(), headers1, 206);

        // Second response: full content with good digest
        let mut headers2 = HashMap::new();
        headers2.insert(
            "Repr-Digest".to_string(),
            format!("sha-256=:{}:", good_digest),
        );
        headers2.insert("ETag".to_string(), "\"NewEtag\"".to_string());
        fetcher.add_response(full_body.to_vec(), headers2, 200);

        let updater = Updater::new(fetcher);
        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        let result_blob = updater.update("remote_path", blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("NewEtag"));
    }

    #[tokio::test]
    async fn test_when_etag_header_is_missing_treats_response_as_update() {
        let fetcher = MockFetcher::default();
        let full_body = b"abc123";

        let headers = HashMap::new(); // No ETag header

        fetcher.add_response(full_body.to_vec(), headers, 200);

        let updater = Updater::new(fetcher);

        // Should not panic or error
        let blob = updater.fetch("remote_path").await.unwrap();
        assert_eq!(blob.content, full_body);
        assert_eq!(blob.etag, None);
    }

    #[tokio::test]
    async fn test_etag_parsing_with_lowercase_header() {
        let fetcher = MockFetcher::default();
        let full_body = b"test content";

        // Simulate real-world server that sends lowercase "etag" header
        let mut headers = HashMap::new();
        headers.insert("etag".to_string(), "\"lowercase-header-etag\"".to_string());

        fetcher.add_response(full_body.to_vec(), headers, 200);

        let updater = Updater::new(fetcher);
        let blob = updater.fetch("remote_path").await.unwrap();

        // ETag should be parsed even though header was lowercase
        assert_eq!(blob.etag(), Some("lowercase-header-etag"));
    }

    #[tokio::test]
    async fn test_fetch_returns_blob_with_metadata() {
        let fetcher = MockFetcher::default();
        let body = b"test content";

        // Calculate correct SHA256 for "test content"
        let hash = sha2::Sha256::digest(body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"etag-123\"".to_string());
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));

        fetcher.add_response(body.to_vec(), headers, 200);

        let updater = Updater::new(fetcher);
        let blob = updater.fetch("remote_path").await.unwrap();

        assert_eq!(blob.content, body);
        assert_eq!(blob.etag(), Some("etag-123"));
        assert_eq!(blob.sha256, Some(digest));
    }

    #[tokio::test]
    async fn test_update_returns_original_blob_when_not_modified() {
        let fetcher = MockFetcher::default();
        let original_body = b"original content";

        fetcher.add_response(vec![], HashMap::new(), 304);

        let updater = Updater::new(fetcher);
        let original_blob = Blob::new(original_body.to_vec()).with_etag("etag-123".to_string());
        let result_blob = updater.update("remote_path", original_blob).await.unwrap();

        assert_eq!(result_blob.content, original_body);
        assert_eq!(result_blob.etag(), Some("etag-123"));
    }

    #[tokio::test]
    async fn test_update_returns_appended_blob_on_partial_content() {
        let fetcher = MockFetcher::default();
        let local_body = b"abc";
        let appended_data = b"c123"; // Note: first byte overlaps
        let full_body = b"abc123";

        let hash = sha2::Sha256::digest(full_body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"new-etag\"".to_string());
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));

        fetcher.add_response(appended_data.to_vec(), headers, 206);

        let updater = Updater::new(fetcher);
        let blob = Blob::new(local_body.to_vec()).with_etag("old-etag".to_string());
        let result_blob = updater.update("remote_path", blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("new-etag"));
    }

    #[tokio::test]
    async fn test_update_returns_full_blob_when_server_ignores_range() {
        let fetcher = MockFetcher::default();
        let full_body = b"complete replacement";

        let hash = sha2::Sha256::digest(full_body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"replacement-etag\"".to_string());
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));

        fetcher.add_response(
            full_body.to_vec(),
            headers,
            200, // Not 206
        );

        let updater = Updater::new(fetcher);
        let blob = Blob::new(b"old content".to_vec());
        let result_blob = updater.update("remote_path", blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("replacement-etag"));
    }
}
