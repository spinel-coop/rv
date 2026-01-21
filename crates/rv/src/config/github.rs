//! GitHub API utilities for rv.
//!
//! This module provides shared functionality for interacting with GitHub's API,
//! including authentication token retrieval.

/// The recommended GitHub API version header value.
/// See: https://docs.github.com/en/rest/overview/api-versions
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

    // Helper to safely manipulate environment variables in tests.
    // Saves original value on creation and restores it on drop.
    // This ensures tests don't pollute each other or leak env changes.
    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        fn new(key: &'static str) -> Self {
            let original = std::env::var(key).ok();
            Self { key, original }
        }

        fn set(&self, value: &str) {
            // SAFETY: We restore the original value in Drop, and each test
            // uses its own guards, so concurrent tests may race but will
            // each restore their expected state.
            unsafe {
                std::env::set_var(self.key, value);
            }
        }

        fn remove(&self) {
            // SAFETY: See set() above.
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: Restoring the original environment state.
            unsafe {
                match &self.original {
                    Some(val) => std::env::set_var(self.key, val),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    fn test_github_token_prefers_github_token_over_gh_token() {
        let github_guard = EnvGuard::new("GITHUB_TOKEN");
        let gh_guard = EnvGuard::new("GH_TOKEN");

        github_guard.set("github_token_value");
        gh_guard.set("gh_token_value");

        assert_eq!(github_token(), Some("github_token_value".to_string()));
    }

    #[test]
    fn test_github_token_falls_back_to_gh_token() {
        let github_guard = EnvGuard::new("GITHUB_TOKEN");
        let gh_guard = EnvGuard::new("GH_TOKEN");

        github_guard.remove();
        gh_guard.set("gh_token_value");

        assert_eq!(github_token(), Some("gh_token_value".to_string()));
    }

    #[test]
    fn test_github_token_returns_none_when_neither_set() {
        let github_guard = EnvGuard::new("GITHUB_TOKEN");
        let gh_guard = EnvGuard::new("GH_TOKEN");

        github_guard.remove();
        gh_guard.remove();

        assert_eq!(github_token(), None);
    }

    #[test]
    fn test_github_token_uses_github_token_when_gh_token_not_set() {
        let github_guard = EnvGuard::new("GITHUB_TOKEN");
        let gh_guard = EnvGuard::new("GH_TOKEN");

        github_guard.set("only_github_token");
        gh_guard.remove();

        assert_eq!(github_token(), Some("only_github_token".to_string()));
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
