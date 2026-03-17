use crate::http_client::rv_http_client;
use async_trait::async_trait;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Response {
    pub body: Vec<u8>,
    pub headers: HashMap<String, String>,
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
        // Case-insensitive header lookup
        let name_lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == name_lower)
            .map(|(_, v)| v.as_str())
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

#[derive(Clone)]
pub struct HttpFetcher {
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl HttpFetcher {
    pub fn new(command: &'static str) -> Result<Self> {
        Ok(Self {
            client: rv_http_client(command)?,
        })
    }
}

/// Trait for fetching remote resources
#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn call(&self, remote_path: &str, headers: HashMap<String, String>) -> Result<Response>;
}

#[async_trait]
impl Fetcher for HttpFetcher {
    /// Make a single HTTP call without retry logic
    async fn call(&self, remote_path: &str, headers: HashMap<String, String>) -> Result<Response> {
        let mut request = self.client.get(remote_path);

        // Add all headers to the request
        for (key, value) in headers {
            request = request.header(&key, value);
        }

        let response = request.send().await?;
        let status_code = response.status().as_u16();

        // Convert response headers to HashMap
        let mut response_headers = HashMap::new();
        for (key, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                response_headers.insert(key.to_string(), value_str.to_string());
            }
        }

        let body = response.bytes().await?.to_vec();

        Ok(Response {
            body,
            headers: response_headers,
            status_code,
        })
    }
}

#[cfg(test)]
// Mock fetcher for testing
pub struct MockFetcher {
    responses: std::sync::Mutex<Vec<Response>>,
}

#[cfg(test)]
impl Default for MockFetcher {
    fn default() -> Self {
        Self {
            responses: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg(test)]
impl MockFetcher {
    pub fn add_response(&self, body: Vec<u8>, headers: HashMap<String, String>, status_code: u16) {
        let response = Response {
            body,
            headers,
            status_code,
        };
        self.responses.lock().unwrap().push(response);
    }
}

#[cfg(test)]
#[async_trait]
impl Fetcher for MockFetcher {
    async fn call(
        &self,
        _remote_path: &str,
        _headers: HashMap<String, String>,
    ) -> Result<Response> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            panic!("No more mock responses available");
        }
        Ok(responses.remove(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_etag_from_response_parses_standard_etag() {
        let mut headers = HashMap::new();
        headers.insert("ETag".to_string(), "\"someetag\"".to_string());
        let response = Response {
            body: vec![],
            headers,
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
            headers,
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
            headers,
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
            headers,
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
            headers: headers1,
            status_code: 200,
        };
        let digests1 = response1.digests().unwrap();
        assert_eq!(digests1.get("sha-256"), Some(&"abc123".to_string()));

        // Test uppercase DIGEST fallback
        let mut headers2 = HashMap::new();
        headers2.insert("DIGEST".to_string(), "sha-256=:xyz789:".to_string());
        let response2 = Response {
            body: vec![],
            headers: headers2,
            status_code: 200,
        };
        let digests2 = response2.digests().unwrap();
        assert_eq!(digests2.get("sha-256"), Some(&"xyz789".to_string()));

        // Test mixed case Repr-Digest
        let mut headers3 = HashMap::new();
        headers3.insert("Repr-Digest".to_string(), "sha-256=:mixed456:".to_string());
        let response3 = Response {
            body: vec![],
            headers: headers3,
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

    #[tokio::test]
    async fn test_get_header_is_case_insensitive() {
        let mut headers = HashMap::new();
        headers.insert("etag".to_string(), "lowercase-etag".to_string());
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        headers.insert("REPR-DIGEST".to_string(), "uppercase-digest".to_string());

        let response = Response {
            body: vec![],
            headers,
            status_code: 200,
        };

        // Should find headers regardless of case
        assert_eq!(response.get_header("ETag"), Some("lowercase-etag"));
        assert_eq!(response.get_header("etag"), Some("lowercase-etag"));
        assert_eq!(response.get_header("ETAG"), Some("lowercase-etag"));

        assert_eq!(response.get_header("content-type"), Some("text/plain"));
        assert_eq!(response.get_header("Content-Type"), Some("text/plain"));
        assert_eq!(response.get_header("CONTENT-TYPE"), Some("text/plain"));

        assert_eq!(response.get_header("repr-digest"), Some("uppercase-digest"));
        assert_eq!(response.get_header("Repr-Digest"), Some("uppercase-digest"));
        assert_eq!(response.get_header("REPR-DIGEST"), Some("uppercase-digest"));

        // Non-existent header
        assert_eq!(response.get_header("X-Missing"), None);
    }
}
