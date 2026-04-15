use reqwest::header::HeaderMap;
use rv_client::http_client::rv_http_client;
use std::collections::HashMap;

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
        // Case-insensitive header lookup
        let name_lower = name.to_lowercase();
        self.headers
            .get(name_lower)
            .and_then(|value| value.to_str().ok())
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

    /// Make a single HTTP call without retry logic
    pub async fn call(&self, remote_path: &str, headers: HeaderMap) -> Result<Response> {
        let request = self.client.get(remote_path).headers(headers);
        let response = request.send().await?.error_for_status()?;
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn test_get_header_is_case_insensitive() {
        let mut headers = HashMap::new();
        headers.insert("etag".to_string(), "lowercase-etag".to_string());
        headers.insert("Content-Type".to_string(), "text/plain".to_string());
        headers.insert("REPR-DIGEST".to_string(), "uppercase-digest".to_string());

        let response = Response {
            body: vec![],
            headers: (&headers).try_into().unwrap(),
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
