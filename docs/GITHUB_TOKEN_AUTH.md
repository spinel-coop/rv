# GitHub Token Authentication in rv

## Overview

rv now supports authenticated GitHub API requests to avoid rate limiting issues when fetching Ruby releases and downloading Ruby tarballs. This is especially important in CI/CD environments where multiple builds may be running concurrently.

## How It Works

rv automatically checks for GitHub tokens in the following order:

1. **`GITHUB_TOKEN`** - Automatically available in GitHub Actions workflows
2. **`GH_TOKEN`** - Used by GitHub CLI and can be set manually

If either token is found, rv will use it for authenticated requests. If no token is found, rv falls back to unauthenticated requests (which are subject to GitHub's stricter rate limits).

## Where Authentication is Used

### 1. Fetching Ruby Releases List

When rv queries the GitHub API for available Ruby versions:
```
https://api.github.com/repos/spinel-coop/rv-ruby/releases/latest
```

**With authentication**: 5,000 requests per hour per user
**Without authentication**: 60 requests per hour per IP address

### 2. Downloading Ruby Tarballs

When rv downloads Ruby binaries from GitHub releases:
```
https://github.com/spinel-coop/rv-ruby/releases/download/...
```

Authentication helps avoid rate limiting when downloading large files.

## Usage Examples

### GitHub Actions (Automatic)

In GitHub Actions, `GITHUB_TOKEN` is automatically available:

```yaml
- name: Install Ruby
  shell: bash
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  run: |
    rv ruby install 3.4
```

### Local Development with GitHub CLI

If you have GitHub CLI (`gh`) installed and authenticated:

```bash
export GH_TOKEN=$(gh auth token)
rv ruby install 3.4
```

### Manual Token Setup

You can create a personal access token and use it:

```bash
export GITHUB_TOKEN="your_token_here"
# or
export GH_TOKEN="your_token_here"

rv ruby install 3.4
```

### Other CI Systems

For CI systems other than GitHub Actions, you can set the token as an environment variable:

**GitLab CI**:
```yaml
install_ruby:
  script:
    - export GITHUB_TOKEN="${GITHUB_TOKEN}"
    - rv ruby install 3.4
  variables:
    GITHUB_TOKEN: $GITHUB_TOKEN
```

**CircleCI**:
```yaml
- run:
    name: Install Ruby
    command: |
      export GITHUB_TOKEN="${GITHUB_TOKEN}"
      rv ruby install 3.4
```

## Benefits

1. **Avoids Rate Limiting**: Authenticated requests have much higher rate limits (5,000/hour vs 60/hour)
2. **More Reliable CI**: Reduces failed builds due to "403 Forbidden" errors
3. **Zero Configuration**: In GitHub Actions, it "just works"
4. **Graceful Fallback**: If no token is available, rv still works (with lower rate limits)

## Troubleshooting

### Still Getting 403 Errors

If you're still seeing rate limiting errors:

1. **Verify token is set**: 
   ```bash
   echo $GITHUB_TOKEN
   echo $GH_TOKEN
   ```

2. **Check token permissions**: The token needs read access to public repositories

3. **Check rate limit status**:
   ```bash
   curl -H "Authorization: Bearer $GITHUB_TOKEN" \
        https://api.github.com/rate_limit
   ```

### Token Not Being Used

rv logs whether it's using authentication:

```
DEBUG rv::config::ruby_fetcher: Using authenticated GitHub API request
```

or

```
DEBUG rv::config::ruby_fetcher: No GitHub token found, using unauthenticated API request
```

Enable debug logging with:
```bash
RUST_LOG=debug rv ruby install 3.4
```

## Security Considerations

- **Never commit tokens**: Don't hardcode tokens in your scripts or configuration files
- **Use secrets**: In CI/CD, always use the platform's secret management (e.g., GitHub Secrets)
- **Minimal permissions**: The token only needs public repository read access
- **Token lifetime**: Consider using short-lived tokens when possible

## Implementation Details

The authentication is implemented in two places in the rv codebase:

1. **`ruby_fetcher.rs`**: Adds authentication to GitHub API requests for release information
2. **`ruby/install.rs`**: Adds authentication when downloading Ruby tarballs from GitHub

Both implementations:
- Check `GITHUB_TOKEN` first, then `GH_TOKEN`
- Only add authentication for GitHub URLs
- Use the `Bearer` token scheme
- Log whether authentication is being used
- Gracefully handle missing tokens

## Related

- [GitHub API Rate Limiting](https://docs.github.com/en/rest/overview/rate-limits-for-the-rest-api)
- [Creating Personal Access Tokens](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token)
- [GITHUB_TOKEN in GitHub Actions](https://docs.github.com/en/actions/security-guides/automatic-token-authentication)
