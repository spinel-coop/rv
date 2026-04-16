use reqwest::header::{HeaderMap, IF_NONE_MATCH, RANGE};
use std::collections::HashMap;
use std::sync::Arc;

use crate::gemserver::storage::{self, Blob, Storage};
use rv_client::registry_client::RegistryClient;

pub struct Updater {
    fetcher: Arc<RegistryClient>,
    storage: Arc<dyn Storage>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    StorageError(#[from] storage::Error),
    #[error(transparent)]
    RegistryClientError(#[from] rv_client::registry_client::Error),
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error("The url {url} unexpectedly returned an empty response")]
    EmptyResponse { url: String },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Updater {
    pub fn new(fetcher: Arc<RegistryClient>, storage: impl Storage + 'static) -> Self {
        Self {
            fetcher,
            storage: Arc::new(storage),
        }
    }

    /// Retrieve dependency information for a given gem
    ///
    /// Uses an incremental cache based on range requests to the registry /info endpoint.
    pub async fn info(&self, gem: &str) -> Result<Blob> {
        let blob = if let Ok(blob) = self.storage.read_blob(gem).await {
            self.update(gem, blob).await?
        } else {
            self.fetch(gem).await?
        };

        self.storage.write_blob(gem, &blob).await?;

        Ok(blob)
    }

    /// Update a file using an existing blob for incremental optimization
    ///
    /// If the blob is empty, automatically falls back to fetch().
    /// Otherwise, tries append first (using blob's etag for conditional request),
    /// falls back to fetch on failure.
    /// Returns the updated blob.
    async fn update(&self, gem: &str, blob: Blob) -> Result<Blob> {
        // Empty blob - go straight to fetch
        if blob.content.is_empty() {
            return self.fetch(gem).await;
        }

        let response = self.fetch_info(gem, Self::request_headers(&blob)).await?;

        // Not modified - nothing to do
        if response.is_not_modified() {
            return Ok(blob);
        }

        // Empty or failed - fall back to fetch
        if response.body.is_empty() {
            return self.fetch(gem).await;
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
                return self.fetch(gem).await;
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
    async fn fetch(&self, gem: &str) -> Result<Blob> {
        // Fetch the file without any conditional headers
        let response = self.fetch_info(gem, HeaderMap::new()).await?;

        // Build the new blob with metadata
        let etag = response.etag();
        let sha256 = response.digests().and_then(|d| d.get("sha-256").cloned());

        if response.body.is_empty() {
            return Err(Error::EmptyResponse {
                url: self.fetcher.info_url(gem).to_string(),
            });
        }

        let new_blob = Blob::with_metadata(response.body, etag, sha256);

        // Verify digest if provided (propagate errors since no fallback available)
        new_blob.verify()?;

        Ok(new_blob)
    }

    fn request_headers(blob: &Blob) -> HeaderMap {
        let mut headers = HeaderMap::with_capacity(2);

        let range = format!("bytes={}-", blob.size().saturating_sub(1));
        headers.insert(RANGE, range.parse().expect("should be valid"));

        if let Some(etag) = blob.etag() {
            headers.insert(
                IF_NONE_MATCH,
                format!("\"{}\"", etag).parse().expect("should be valid"),
            );
        };

        headers
    }

    async fn fetch_info(&self, gem: &str, headers: HeaderMap) -> Result<Response> {
        let response = self.fetcher.get_info(gem, headers).await?;

        let status_code = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response.bytes().await?.to_vec();

        Ok(Response {
            body,
            headers,
            status_code,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub body: Vec<u8>,
    pub headers: HeaderMap,
    pub status_code: u16,
}

impl Response {
    pub fn is_not_modified(&self) -> bool {
        self.status_code == 304
    }

    pub fn is_partial_content(&self) -> bool {
        self.status_code == 206
    }

    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    pub fn etag(&self) -> Option<String> {
        let etag = self.get_header("ETag")?;
        let etag = etag.strip_prefix("W/").unwrap_or(etag);

        strip_wrapper(etag, '"').map(|s| s.to_string())
    }

    pub fn digests(&self) -> Option<HashMap<String, String>> {
        let header = self
            .get_header("Repr-Digest")
            .or_else(|| self.get_header("Digest"))?;

        let digests: HashMap<String, String> = header
            .split(',')
            .filter_map(|param| {
                let (algo, value) = param.split_once('=')?;
                let algorithm = algo.trim().to_lowercase();

                if algorithm == "sha-256" {
                    byte_sequence(value.trim()).map(|v| (algorithm, v))
                } else {
                    None
                }
            })
            .collect();

        (!digests.is_empty()).then_some(digests)
    }
}

fn byte_sequence(value: &str) -> Option<String> {
    strip_wrapper(value, ':')
        .or_else(|| strip_wrapper(value, '"'))
        .map(|s| s.to_string())
}

fn strip_wrapper(s: &str, wrapper: char) -> Option<&str> {
    s.strip_prefix(wrapper)
        .and_then(|s| s.strip_suffix(wrapper))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gemserver::storage::{self, FilesystemStorage};
    use base64::Engine;
    use sha2::Digest;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_when_local_path_does_not_exist_downloads_file_without_attempting_append() {
        let full_body = b"abc123";

        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"thisisanetag\"".to_string());

        let (server, _mock) = mock_info(full_body, headers, 200).await;

        let storage = dummy_storage();
        let blob = dummy_updater(&server, storage).info("foo").await.unwrap();

        assert_eq!(blob.content, full_body);
        assert_eq!(blob.etag(), Some("thisisanetag"));
    }

    #[tokio::test]
    async fn test_when_local_path_does_not_exist_fails_immediately_on_bad_checksum() {
        let full_body = b"abc123";

        let mut headers = HashMap::new();
        headers.insert("Repr-Digest".to_string(), "sha-256=:baddigest:".to_string());
        headers.insert("ETag".to_string(), "\"thisisanetag\"".to_string());

        let (server, _mock) = mock_info(full_body, headers, 200).await;

        let storage = dummy_storage();
        let result = dummy_updater(&server, storage).info("foo").await;

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

        let (server, _mock) = mock_info(local_body, headers, 304).await; // Not modified

        let storage = dummy_storage();
        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        storage.write_blob("foo", &blob).await.unwrap();
        let result_blob = dummy_updater(&server, storage).info("foo").await.unwrap();

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

        let (server, _mock) = mock_info(
            b"c123", // Partial content (skipping first 2 bytes)
            headers, 206, // Partial Content
        )
        .await;

        let storage = dummy_storage();
        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        storage.write_blob("foo", &blob).await.unwrap();
        let result_blob = dummy_updater(&server, storage).info("foo").await.unwrap();

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

        let (server, _mock) = mock_info(
            full_body, headers, 200, // Full response, not partial
        )
        .await;

        let storage = dummy_storage();
        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        storage.write_blob("foo", &blob).await.unwrap();
        let result_blob = dummy_updater(&server, storage).info("foo").await.unwrap();

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

        let storage = dummy_storage();

        let blob = Blob::new(local_body.to_vec()).with_etag("LocalEtag".to_string());
        storage.write_blob("foo", &blob).await.unwrap();
        let result_blob = dummy_updater(&server, storage).info("foo").await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("NewEtag"));

        mock.assert();
        full_mock.assert();
    }

    #[tokio::test]
    async fn test_when_etag_header_is_missing_treats_response_as_update() {
        let full_body = b"abc123";

        let headers = HashMap::new(); // No ETag header

        let (server, _mock) = mock_info(full_body, headers, 200).await;

        let storage = dummy_storage();

        // Should not panic or error
        let blob = dummy_updater(&server, storage).info("foo").await.unwrap();
        assert_eq!(blob.content, full_body);
        assert_eq!(blob.etag, None);
    }

    #[tokio::test]
    async fn test_etag_parsing_with_lowercase_header() {
        let full_body = b"test content";

        // Simulate real-world server that sends lowercase "etag" header
        let mut headers = HashMap::new();
        headers.insert("etag".to_string(), "\"lowercase-header-etag\"".to_string());

        let (server, _mock) = mock_info(full_body, headers, 200).await;
        let storage = dummy_storage();
        let blob = dummy_updater(&server, storage).info("foo").await.unwrap();

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

        let (server, _mock) = mock_info(body, headers, 200).await;

        let storage = dummy_storage();
        let blob = dummy_updater(&server, storage).info("foo").await.unwrap();

        assert_eq!(blob.content, body);
        assert_eq!(blob.etag(), Some("etag-123"));
        assert_eq!(blob.sha256, Some(digest));
    }

    #[tokio::test]
    async fn test_update_returns_original_blob_when_not_modified() {
        let mut server = mockito::Server::new_async().await;
        let original_body = b"original content";

        let mock = server
            .mock("GET", "/info/foo")
            .with_status(304) // Not Modified
            .create();

        let storage = dummy_storage();
        let original_blob = Blob::new(original_body.to_vec()).with_etag("etag-123".to_string());
        storage.write_blob("foo", &original_blob).await.unwrap();
        let result_blob = dummy_updater(&server, storage).info("foo").await.unwrap();

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

        let (server, _mock) = mock_info(appended_data, headers, 206).await;

        let storage = dummy_storage();
        let blob = Blob::new(local_body.to_vec()).with_etag("old-etag".to_string());
        storage.write_blob("foo", &blob).await.unwrap();
        let result_blob = dummy_updater(&server, storage).info("foo").await.unwrap();

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

        let (server, _mock) = mock_info(
            full_body, headers, 200, // Not 206
        )
        .await;

        let storage = dummy_storage();
        let blob = Blob::new(b"old content".to_vec());
        storage.write_blob("info/foo", &blob).await.unwrap();
        let result_blob = dummy_updater(&server, storage).info("foo").await.unwrap();

        assert_eq!(result_blob.content, full_body);
        assert_eq!(result_blob.etag(), Some("replacement-etag"));
    }

    fn dummy_storage() -> FilesystemStorage {
        let temp_dir = TempDir::new().unwrap();
        FilesystemStorage::new(temp_dir.path().to_path_buf())
    }

    fn dummy_updater(server: &mockito::Server, storage: FilesystemStorage) -> Updater {
        let client = Arc::new(RegistryClient::new(server.url().as_str(), "dummy").unwrap());
        Updater::new(client, storage)
    }

    async fn mock_info(
        body: &[u8],
        headers: HashMap<String, String>,
        status: usize,
    ) -> (mockito::Server, mockito::Mock) {
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

        (server, mock.create())
    }

    #[tokio::test]
    async fn test_etag_from_response_parses_standard_etag() {
        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"someetag\"".to_string());
        let response = Response {
            body: vec![],
            headers: (&headers).try_into().unwrap(),
            status_code: 200,
        };

        assert_eq!(response.etag(), Some("someetag".to_string()));
    }

    #[tokio::test]
    async fn test_etag_from_response_handles_weak_etag() {
        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "W/\"someetag\"".to_string());
        let response = Response {
            body: vec![],
            headers: (&headers).try_into().unwrap(),
            status_code: 200,
        };

        assert_eq!(response.etag(), Some("someetag".to_string()));
    }

    #[tokio::test]
    async fn test_parse_digests_with_repr_digest() {
        let mut headers = HashMap::new();
        headers.insert("Repr-Digest".to_string(), "sha-256=:abcd1234:".to_string());
        let response = Response {
            body: vec![],
            headers: (&headers).try_into().unwrap(),
            status_code: 200,
        };

        let digests = response.digests().unwrap();
        assert_eq!(digests.get("sha-256"), Some(&"abcd1234".to_string()));
    }

    #[tokio::test]
    async fn test_parse_digests_with_digest_header() {
        let mut headers = HashMap::new();
        headers.insert("Digest".to_string(), "sha-256=:abcd1234:".to_string());
        let response = Response {
            body: vec![],
            headers: (&headers).try_into().unwrap(),
            status_code: 200,
        };

        let digests = response.digests().unwrap();
        assert_eq!(digests.get("sha-256"), Some(&"abcd1234".to_string()));
    }

    #[tokio::test]
    async fn test_digest_parsing_with_various_header_cases() {
        // Test lowercase repr-digest
        let mut headers1 = HashMap::new();
        headers1.insert("repr-digest".to_string(), "sha-256=:abc123:".to_string());
        let response1 = Response {
            body: vec![],
            headers: (&headers1).try_into().unwrap(),
            status_code: 200,
        };
        let digests1 = response1.digests().unwrap();
        assert_eq!(digests1.get("sha-256"), Some(&"abc123".to_string()));

        // Test uppercase DIGEST fallback
        let mut headers2 = HashMap::new();
        headers2.insert("DIGEST".to_string(), "sha-256=:xyz789:".to_string());
        let response2 = Response {
            body: vec![],
            headers: (&headers2).try_into().unwrap(),
            status_code: 200,
        };
        let digests2 = response2.digests().unwrap();
        assert_eq!(digests2.get("sha-256"), Some(&"xyz789".to_string()));

        // Test mixed case Repr-Digest
        let mut headers3 = HashMap::new();
        headers3.insert("Repr-Digest".to_string(), "sha-256=:mixed456:".to_string());
        let response3 = Response {
            body: vec![],
            headers: (&headers3).try_into().unwrap(),
            status_code: 200,
        };
        let digests3 = response3.digests().unwrap();
        assert_eq!(digests3.get("sha-256"), Some(&"mixed456".to_string()));
    }

    #[tokio::test]
    async fn test_byte_sequence_unwraps_colons() {
        assert_eq!(byte_sequence(":value:"), Some("value".to_string()));
    }

    #[tokio::test]
    async fn test_byte_sequence_unwraps_quotes() {
        assert_eq!(byte_sequence("\"value\""), Some("value".to_string()));
    }

    #[tokio::test]
    async fn test_byte_sequence_returns_none_for_mismatched_wrappers() {
        assert_eq!(byte_sequence(":value\""), None);
        assert_eq!(byte_sequence("\"value:"), None);
        assert_eq!(byte_sequence(":value"), None);
        assert_eq!(byte_sequence("value:"), None);
    }
}
