//! GitHub API utilities for rv.
//!
//! This module provides shared functionality for interacting with GitHub's API,
//! including authentication token retrieval.

/// The recommended GitHub API version header value.
/// See: <https://docs.github.com/en/rest/overview/api-versions>
pub const GITHUB_API_VERSION: &str = "2022-11-28";

/// Retrieves a GitHub authentication token from environment variables.
///
/// Checks `GITHUB_TOKEN` first (automatically available in GitHub Actions),
/// then falls back to `GH_TOKEN` (used by GitHub CLI and for general use).
///
/// Returns `None` if neither environment variable is set.
pub fn github_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN")
        .ok()
        .or_else(|| std::env::var("GH_TOKEN").ok())
}

/// Builds a `reqwest::RequestBuilder` for a GitHub API endpoint with standard headers
/// and optional authentication.
pub fn github_api_get(
    client: &reqwest::Client,
    url: impl reqwest::IntoUrl,
) -> reqwest::RequestBuilder {
    use tracing::debug;

    let mut builder = client
        .get(url)
        .header("User-Agent", "rv-cli")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", GITHUB_API_VERSION);

    if let Some(token) = github_token() {
        debug!("Using authenticated GitHub API request");
        builder = builder.header("Authorization", format!("Bearer {}", token));
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

    #[test]
    fn test_github_token_prefers_github_token_over_gh_token() {
        unsafe {
            std::env::set_var("GH_TOKEN", "gh_token_value");
            assert_eq!(github_token(), Some("gh_token_value".to_string()));
            std::env::set_var("GITHUB_TOKEN", "github_token_value");
            assert_eq!(github_token(), Some("github_token_value".to_string()));
        }
    }

    #[test]
    fn test_github_token_falls_back_to_gh_token() {
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
            assert_eq!(github_token(), None);
            std::env::set_var("GH_TOKEN", "gh_token_value");
            assert_eq!(github_token(), Some("gh_token_value".to_string()));
        }
    }

    #[test]
    fn test_github_token_returns_none_when_neither_set() {
        unsafe {
            std::env::remove_var("GITHUB_TOKEN");
            std::env::remove_var("GH_TOKEN");
            assert_eq!(github_token(), None);
        }
    }

    #[test]
    fn test_github_token_uses_github_token_when_gh_token_not_set() {
        unsafe {
            std::env::set_var("GITHUB_TOKEN", "only_github_token");
            std::env::remove_var("GH_TOKEN");
            assert_eq!(github_token(), Some("only_github_token".to_string()));
        }
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
