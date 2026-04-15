use std::collections::HashMap;
use std::sync::Arc;

use crate::gemserver::Error;
use crate::gemserver::http_fetcher::HttpFetcher;
use crate::gemserver::storage::Blob;

pub type Result<T> = std::result::Result<T, Error>;

pub struct Updater {
    fetcher: Arc<HttpFetcher>,
}

impl Updater {
    pub fn new(fetcher: HttpFetcher) -> Self {
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

        let response = self
            .fetcher
            .call(remote_path, Self::request_headers(&blob))
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

        if response.body.is_empty() {
            return Err(Error::EmptyResponse {
                url: remote_path.to_owned(),
            });
        }

        let new_blob = Blob::with_metadata(response.body, etag, sha256);

        // Verify digest if provided (propagate errors since no fallback available)
        new_blob.verify()?;

        Ok(new_blob)
    }

    fn request_headers(blob: &Blob) -> HashMap<String, String> {
        let mut headers = HashMap::from([(
            "Range".to_string(),
            format!("bytes={}-", blob.size().saturating_sub(1) as usize),
        )]);

        if let Some(etag) = blob.etag() {
            headers.insert("If-None-Match".to_string(), format!("\"{}\"", etag));
        };

        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gemserver::storage;
    use base64::Engine;
    use sha2::Digest;

    #[tokio::test]
    async fn test_when_local_path_does_not_exist_downloads_file_without_attempting_append() {
        let full_body = b"abc123";

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"thisisanetag\"".to_string());

        let (remote_path, _server, _mock) = mock_info(full_body, headers, 200).await;

        let blob = dummy_updater().fetch(&remote_path).await.unwrap();

        assert_eq!(blob.content, full_body);
        assert_eq!(blob.etag(), Some("thisisanetag"));
    }

    #[tokio::test]
    async fn test_when_local_path_does_not_exist_fails_immediately_on_bad_checksum() {
        let full_body = b"abc123";

        let mut headers = HashMap::new();
        headers.insert("Repr-Digest".to_string(), "sha-256=:baddigest:".to_string());
        headers.insert("ETag".to_string(), "\"thisisanetag\"".to_string());

        let (remote_path, _server, _mock) = mock_info(full_body, headers, 200).await;

        let result = dummy_updater().fetch(&remote_path).await;

        assert!(matches!(
            result,
            Err(Error::StorageError(
                storage::Error::MismatchedChecksum { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn test_when_local_path_exists_with_etag_does_nothing_if_etags_match() {
        let local_body = b"abc";

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"LocalEtag\"".to_string());

        let (remote_path, _server, _mock) = mock_info(local_body, headers, 304).await; // Not modified

        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        let result_blob = dummy_updater().update(&remote_path, blob).await.unwrap();

        assert_eq!(result_blob.content, local_body);
        assert_eq!(result_blob.etag(), Some("LocalEtag"));
    }

    #[tokio::test]
    async fn test_when_local_path_exists_with_etag_appends_file_if_etags_do_not_match() {
        let local_body = b"abc";
        let full_body = b"abc123";
        let hash = sha2::Sha256::digest(full_body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));
        headers.insert("ETag".to_string(), "\"NewEtag\"".to_string());

        let (remote_path, _server, _mock) = mock_info(
            b"c123", // Partial content (skipping first 2 bytes)
            headers, 206, // Partial Content
        )
        .await;

        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        let result_blob = dummy_updater().update(&remote_path, blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("NewEtag"));
    }

    #[tokio::test]
    async fn test_when_local_path_exists_with_etag_replaces_file_if_response_ignores_range() {
        let local_body = b"abc";
        let full_body = b"abc123";
        let hash = sha2::Sha256::digest(full_body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));
        headers.insert("ETag".to_string(), "\"NewEtag\"".to_string());

        let (remote_path, _server, _mock) = mock_info(
            full_body, headers, 200, // Full response, not partial
        )
        .await;

        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        let result_blob = dummy_updater().update(&remote_path, blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("NewEtag"));
    }

    #[tokio::test]
    async fn test_tries_request_again_if_partial_response_fails_digest_check() {
        let mut server = mockito::Server::new_async().await;
        let local_body = b"abc";
        let full_body = b"abc123";
        let hash = sha2::Sha256::digest(full_body);
        let good_digest = base64::engine::general_purpose::STANDARD.encode(hash);

        // First response: partial content with bad digest
        let mock = server
            .mock("GET", "/info/foo")
            .with_body(b"the beginning of the file changed")
            .with_header("Repr-Digest", "sha-256=:baddigest:")
            .with_status(206)
            .create();

        // Second response: full content with good digest
        let full_mock = server
            .mock("GET", "/info/foo")
            .with_body(full_body)
            .with_header("Repr-Digest", &format!("sha-256=:{}:", good_digest))
            .with_header("ETag", "\"NewEtag\"")
            .with_status(200)
            .create();

        let remote_path = format!("{}/info/foo", server.url());
        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        let result_blob = dummy_updater().update(&remote_path, blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("NewEtag"));

        mock.assert();
        full_mock.assert();
    }

    #[tokio::test]
    async fn test_when_etag_header_is_missing_treats_response_as_update() {
        let full_body = b"abc123";

        let headers = HashMap::new(); // No ETag header

        let (remote_path, _server, _mock) = mock_info(full_body, headers, 200).await;

        // Should not panic or error
        let blob = dummy_updater().fetch(&remote_path).await.unwrap();
        assert_eq!(blob.content, full_body);
        assert_eq!(blob.etag, None);
    }

    #[tokio::test]
    async fn test_etag_parsing_with_lowercase_header() {
        let full_body = b"test content";

        // Simulate real-world server that sends lowercase "etag" header
        let mut headers = HashMap::new();
        headers.insert("etag".to_string(), "\"lowercase-header-etag\"".to_string());

        let (remote_path, _server, _mock) = mock_info(full_body, headers, 200).await;

        let blob = dummy_updater().fetch(&remote_path).await.unwrap();

        // ETag should be parsed even though header was lowercase
        assert_eq!(blob.etag(), Some("lowercase-header-etag"));
    }

    #[tokio::test]
    async fn test_fetch_returns_blob_with_metadata() {
        let body = b"test content";

        // Calculate correct SHA256 for "test content"
        let hash = sha2::Sha256::digest(body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"etag-123\"".to_string());
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));

        let (remote_path, _server, _mock) = mock_info(body, headers, 200).await;

        let blob = dummy_updater().fetch(&remote_path).await.unwrap();

        assert_eq!(blob.content, body);
        assert_eq!(blob.etag(), Some("etag-123"));
        assert_eq!(blob.sha256, Some(digest));
    }

    #[tokio::test]
    async fn test_update_returns_original_blob_when_not_modified() {
        let mut server = mockito::Server::new_async().await;
        let original_body = b"original content";

        let mock = server.mock("GET", "/info/foo").with_status(304).create();

        let remote_path = format!("{}/info/foo", server.url());
        let original_blob = Blob::new(original_body.to_vec()).with_etag("etag-123".to_string());
        let result_blob = dummy_updater()
            .update(&remote_path, original_blob)
            .await
            .unwrap();

        assert_eq!(result_blob.content, original_body);
        assert_eq!(result_blob.etag(), Some("etag-123"));

        mock.assert();
    }

    #[tokio::test]
    async fn test_update_returns_appended_blob_on_partial_content() {
        let local_body = b"abc";
        let appended_data = b"c123"; // Note: first byte overlaps
        let full_body = b"abc123";

        let hash = sha2::Sha256::digest(full_body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"new-etag\"".to_string());
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));

        let (remote_path, _server, _mock) = mock_info(appended_data, headers, 206).await;

        let blob = Blob::new(local_body.to_vec()).with_etag("old-etag".to_string());
        let result_blob = dummy_updater().update(&remote_path, blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("new-etag"));
    }

    #[tokio::test]
    async fn test_update_returns_full_blob_when_server_ignores_range() {
        let full_body = b"complete replacement";

        let hash = sha2::Sha256::digest(full_body);
        let digest = base64::engine::general_purpose::STANDARD.encode(hash);

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"replacement-etag\"".to_string());
        headers.insert("Repr-Digest".to_string(), format!("sha-256=:{}:", digest));

        let (remote_path, _server, _mock) = mock_info(
            full_body, headers, 200, // Not 206
        )
        .await;

        let blob = Blob::new(b"old content".to_vec());
        let result_blob = dummy_updater().update(&remote_path, blob).await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("replacement-etag"));
    }

    fn dummy_updater() -> Updater {
        let client = HttpFetcher::new("dummy").unwrap();
        Updater::new(client)
    }

    async fn mock_info(
        body: &[u8],
        headers: HashMap<String, String>,
        status: usize,
    ) -> (String, mockito::Server, mockito::Mock) {
        let opts = mockito::ServerOpts {
            assert_on_drop: true,
            ..Default::default()
        };
        let mut server = mockito::Server::new_with_opts_async(opts).await;

        let mut mock = server.mock("GET", "/info/foo");

        for (name, value) in headers {
            mock = mock.with_header(name, &value);
        }

        mock = mock.with_status(status).with_body(body);

        (format!("{}/info/foo", server.url()), server, mock.create())
    }
}
