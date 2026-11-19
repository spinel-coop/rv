//! GitHub API utilities for rv.
//!
//! This module provides shared functionality for interacting with GitHub's API,
//! including authentication token retrieval.

/// The recommended GitHub API version header value.
/// See: <https://docs.github.com/en/rest/overview/api-versions>
pub const GITHUB_API_VERSION: &str = "2022-11-28";

/// User-Agent sent on requests to GitHub, including the rv version.
pub const GITHUB_USER_AGENT: &str = concat!("rv/", env!("CARGO_PKG_VERSION"));

/// Environment variables that may hold a GitHub token, highest priority first.
///
/// `RV_GITHUB_TOKEN` is rv-specific and overrides the others; `GITHUB_TOKEN` is
/// provided automatically in GitHub Actions; `GH_TOKEN` is used by the GitHub CLI.
const GITHUB_TOKEN_ENV_VARS: [&str; 3] = ["RV_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"];

/// Retrieves a GitHub authentication token from environment variables.
///
/// Checks `RV_GITHUB_TOKEN`, then `GITHUB_TOKEN`, then `GH_TOKEN`, and returns the
/// first non-empty value with surrounding whitespace trimmed.
///
/// Returns `None` if no token is configured.
pub fn github_token() -> Option<String> {
    github_token_from(|name| std::env::var(name).ok())
}

fn github_token_from(lookup: impl Fn(&str) -> Option<String>) -> Option<String> {
    GITHUB_TOKEN_ENV_VARS.iter().find_map(|name| {
        lookup(name)
            .map(|token| token.trim().to_owned())
            .filter(|token| !token.is_empty())
    })
}

/// Builds a `reqwest::RequestBuilder` for a GitHub API endpoint with standard headers
/// and optional authentication.
pub fn github_api_get(
    client: &reqwest::Client,
    url: impl reqwest::IntoUrl,
) -> reqwest::RequestBuilder {
    github_api_get_with_token(client, url, github_token().as_deref())
}

fn github_api_get_with_token(
    client: &reqwest::Client,
    url: impl reqwest::IntoUrl,
    token: Option<&str>,
) -> reqwest::RequestBuilder {
    use tracing::debug;

    let mut builder = client
        .get(url)
        .header("User-Agent", GITHUB_USER_AGENT)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", GITHUB_API_VERSION);

    if let Some(token) = token {
        debug!("Using authenticated GitHub API request");
        builder = builder.bearer_auth(token);
    } else {
        debug!("No GitHub token found, using unauthenticated API request");
    }

    builder
}

/// Checks if the URL is a GitHub URL by parsing the host.
/// Returns true if the host is exactly "github.com" or ends with ".github.com".
pub fn is_github_url(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        .is_some_and(|host| host == "github.com" || host.ends_with(".github.com"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_api_version_is_valid() {
        // Ensure the API version follows the expected format (YYYY-MM-DD)
        assert_eq!(GITHUB_API_VERSION.len(), 10);
        assert!(GITHUB_API_VERSION.chars().nth(4) == Some('-'));
        assert!(GITHUB_API_VERSION.chars().nth(7) == Some('-'));
    }

    fn token_from(vars: &[(&str, &str)]) -> Option<String> {
        github_token_from(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn test_github_token_prefers_rv_github_token() {
        assert_eq!(
            token_from(&[
                ("RV_GITHUB_TOKEN", "rv_token_value"),
                ("GITHUB_TOKEN", "github_token_value"),
                ("GH_TOKEN", "gh_token_value"),
            ]),
            Some("rv_token_value".to_string())
        );
    }

    #[test]
    fn test_github_token_prefers_github_token_over_gh_token() {
        assert_eq!(
            token_from(&[
                ("GITHUB_TOKEN", "github_token_value"),
                ("GH_TOKEN", "gh_token_value"),
            ]),
            Some("github_token_value".to_string())
        );
    }

    #[test]
    fn test_github_token_falls_back_to_gh_token() {
        assert_eq!(
            token_from(&[("GH_TOKEN", "gh_token_value")]),
            Some("gh_token_value".to_string())
        );
    }

    #[test]
    fn test_github_token_returns_none_when_unset() {
        assert_eq!(token_from(&[]), None);
    }

    #[test]
    fn test_github_token_skips_empty_values() {
        assert_eq!(
            token_from(&[
                ("RV_GITHUB_TOKEN", ""),
                ("GITHUB_TOKEN", "   "),
                ("GH_TOKEN", "gh_token_value"),
            ]),
            Some("gh_token_value".to_string())
        );
        assert_eq!(
            token_from(&[
                ("RV_GITHUB_TOKEN", ""),
                ("GITHUB_TOKEN", ""),
                ("GH_TOKEN", ""),
            ]),
            None
        );
    }

    #[test]
    fn test_github_token_trims_whitespace() {
        assert_eq!(
            token_from(&[("RV_GITHUB_TOKEN", "  rv_token_value\n")]),
            Some("rv_token_value".to_string())
        );
    }

    fn api_request_headers(token: Option<&str>) -> reqwest::header::HeaderMap {
        github_api_get_with_token(
            &reqwest::Client::new(),
            "https://api.github.com/repos/spinel-coop/rv-ruby/releases/latest",
            token,
        )
        .build()
        .expect("request should build")
        .headers()
        .clone()
    }

    #[test]
    fn test_github_api_get_sends_bearer_token() {
        let headers = api_request_headers(Some("secret-token"));
        assert_eq!(
            headers[reqwest::header::AUTHORIZATION],
            "Bearer secret-token"
        );
    }

    #[test]
    fn test_github_api_get_omits_authorization_without_token() {
        let headers = api_request_headers(None);
        assert!(!headers.contains_key(reqwest::header::AUTHORIZATION));
    }

    #[test]
    fn test_github_api_get_sets_standard_headers() {
        let headers = api_request_headers(None);
        assert_eq!(
            headers[reqwest::header::USER_AGENT],
            format!("rv/{}", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(
            headers[reqwest::header::ACCEPT],
            "application/vnd.github+json"
        );
        assert_eq!(headers["x-github-api-version"], GITHUB_API_VERSION);
    }

    #[test]
    fn test_is_github_url_exact_match() {
        assert!(is_github_url("https://github.com/owner/repo"));
        assert!(is_github_url("http://github.com/owner/repo"));
        assert!(is_github_url(
            "https://github.com/spinel-coop/rv-ruby/releases/latest/download/ruby-3.3.0.tar.gz"
        ));
    }

    #[test]
    fn test_is_github_url_subdomains() {
        assert!(is_github_url("https://api.github.com/repos/owner/repo"));
        assert!(is_github_url("https://raw.github.com/owner/repo/main/file"));
        assert!(is_github_url("https://objects.github.com/something"));
    }

    #[test]
    fn test_is_github_url_case_insensitive() {
        assert!(is_github_url("https://GITHUB.COM/owner/repo"));
        assert!(is_github_url("https://GitHub.com/owner/repo"));
        assert!(is_github_url("https://API.GITHUB.COM/repos"));
    }

    #[test]
    fn test_is_github_url_rejects_fake_domains() {
        // These should NOT match - they're not actually github.com
        assert!(!is_github_url("https://fakegithub.com/owner/repo"));
        assert!(!is_github_url("https://notgithub.com/owner/repo"));
        assert!(!is_github_url("https://github.com.evil.com/owner/repo"));
        assert!(!is_github_url("https://mygithub.company.com/owner/repo"));
    }

    #[test]
    fn test_is_github_url_rejects_github_in_path() {
        // github.com in path should not match
        assert!(!is_github_url("https://example.com/github.com/owner/repo"));
        assert!(!is_github_url(
            "https://mirror.example.com/proxy/github.com/file"
        ));
    }

    #[test]
    fn test_is_github_url_rejects_non_github() {
        assert!(!is_github_url("https://gitlab.com/owner/repo"));
        assert!(!is_github_url("https://bitbucket.org/owner/repo"));
        assert!(!is_github_url("https://example.com/file.tar.gz"));
    }

    #[test]
    fn test_is_github_url_handles_invalid_urls() {
        assert!(!is_github_url("not a url"));
        assert!(!is_github_url(""));
        assert!(!is_github_url("github.com/owner/repo")); // missing scheme
    }
}
