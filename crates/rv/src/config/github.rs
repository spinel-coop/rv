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
}
