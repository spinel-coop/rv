use std::collections::BTreeMap;
use std::io;
use std::time::{Duration, SystemTime};

use anstream::println;
use fs_err as fs;
use owo_colors::OwoColorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::config::Config;

// Use GitHub's TTL, but don't re-check more than every 60 seconds.
const MINIMUM_CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error("Failed to fetch available ruby versions from GitHub")]
    RequestError(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

type Result<T> = miette::Result<T, Error>;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Release {
    name: String,
}

// Updated struct to hold ETag and calculated expiry time
#[derive(Serialize, Deserialize, Debug)]
struct CachedReleases {
    expires_at: SystemTime,
    etag: Option<String>,
    releases: Vec<Release>,
}

/// Parses the `max-age` value from a `Cache-Control` header.
fn parse_max_age(header: &str) -> Option<Duration> {
    let re = Regex::new(r"max-age=(\d+)").unwrap();
    re.captures(header)
        .and_then(|caps| caps.get(1))
        .and_then(|age| age.as_str().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Fetches available rubies
async fn fetch_available_rubies(cache: &rv_cache::Cache) -> Result<Vec<String>> {
    let cache_entry = cache.entry(
        rv_cache::CacheBucket::Ruby,
        "releases",
        "available_rubies.json",
    );
    let client = reqwest::Client::new();
    let url = "https://api.github.com/repos/spinel-coop/rv-ruby/releases";

    // 1. Try to read from the disk cache.
    let cached_data: Option<CachedReleases> =
        if let Ok(content) = fs::read_to_string(cache_entry.path()) {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

    // 2. If we have fresh cached data, use it immediately.
    if let Some(cache) = &cached_data {
        if SystemTime::now() < cache.expires_at {
            debug!("Using cached list of available rubies.");
            return Ok(cache.releases.clone().into_iter().map(|r| r.name).collect());
        }
        debug!("Cached ruby list is stale, re-validating with server.");
    }

    // 3. Cache is stale or missing
    let etag = cached_data.as_ref().and_then(|c| c.etag.clone());
    let mut request_builder = client
        .get(url)
        .header("User-Agent", "rv-cli")
        .header("Accept", "application/vnd.github+json");

    // 4. Use ETag for conditional requests if we have one
    if let Some(etag) = &etag {
        debug!("Using ETag to make a conditional request: {}", etag);
        request_builder = request_builder.header("If-None-Match", etag.clone());
    }

    let response = request_builder.send().await?;

    // 4. Handle the server's response.
    match response.status() {
        reqwest::StatusCode::NOT_MODIFIED => {
            debug!("GitHub API confirmed releases list is unchanged (304 Not Modified).");
            let mut stale_cache = cached_data.ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "304 response without prior cache")
            })?;

            // Update the expiry time based on the latest Cache-Control header
            let max_age = response
                .headers()
                .get("Cache-Control")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_max_age)
                .unwrap_or(Duration::from_secs(60));
            
            stale_cache.expires_at = SystemTime::now() + max_age.max(MINIMUM_CACHE_TTL);
            fs::write(cache_entry.path(), serde_json::to_string(&stale_cache)?)?;

            Ok(stale_cache.releases.into_iter().map(|r| r.name).collect())
        }
        reqwest::StatusCode::OK => {
            debug!("Received new releases list from GitHub (200 OK).");
            let headers = response.headers().clone();
            let new_etag = headers.get("ETag").and_then(|v| v.to_str().ok()).map(String::from);
            
            let max_age = headers
                .get("Cache-Control")
                .and_then(|v| v.to_str().ok())
                .and_then(parse_max_age)
                .unwrap_or(Duration::from_secs(60)); // Default to 60s if header is missing
            
            let releases: Vec<Release> = response.json().await?;

            let new_cache_entry = CachedReleases {
                expires_at: SystemTime::now() + max_age.max(MINIMUM_CACHE_TTL),
                etag: new_etag,
                releases: releases.clone(),
            };

            if let Some(parent) = cache_entry.path().parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(cache_entry.path(), serde_json::to_string(&new_cache_entry)?)?;

            Ok(releases.into_iter().map(|r| r.name).collect())
        }
        status => {
            warn!("Failed to fetch releases, status: {}", status);
            Err(response.error_for_status().unwrap_err().into())
        }
    }
}

/// Lists the available and installed rubies.
pub async fn list(config: &Config, format: OutputFormat, installed_only: bool) -> Result<()> {
    let installed_rubies = config.rubies();
    let active = config.project_ruby();

    if installed_only {
        if installed_rubies.is_empty() {
            warn!("No Ruby installations found.");
            info!("Try installing Ruby with 'rv ruby install <version>'");
        } else {
            print_rubies(&installed_rubies, &active, format)?;
        }
        return Ok(());
    }

    // Pass the cache from the config to the fetch function
    let available_rubies = match fetch_available_rubies(&config.cache).await {
        Ok(rubies) => rubies,
        Err(e) => {
            warn!("Could not fetch or re-validate available Ruby versions: {}", e);
            // On failure, try to read from a potentially stale cache as a fallback
            let cache_entry = config.cache.entry(
                rv_cache::CacheBucket::Ruby,
                "releases",
                "available_rubies.json",
            );
            if let Ok(content) = fs::read_to_string(cache_entry.path()) {
                if let Ok(cached_data) = serde_json::from_str::<CachedReleases>(&content) {
                    warn!("Displaying stale list of available rubies from cache.");
                    cached_data.releases.into_iter().map(|r| r.name).collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        }
    };

    // Normalize GitHub versions to include the "ruby-" prefix
    let mut combined: BTreeMap<String, Option<&rv_ruby::Ruby>> = available_rubies
        .into_iter()
        .map(|name| (format!("ruby-{}", name), None))
        .collect();

    for ruby in &installed_rubies {
        combined.insert(ruby.display_name(), Some(ruby));
    }

    if combined.is_empty() {
        warn!("No rubies found.");
        info!("Try installing a ruby with 'rv ruby install <version>'.");
        return Ok(());
    }

    match format {
        OutputFormat::Text => {
            let width = combined
                .keys()
                .map(|name| name.len())
                .max()
                .unwrap_or_default();

            for (name, maybe_ruby) in &combined {
                let entry = format_ruby_entry(name, maybe_ruby, &active, width);
                println!("{entry}");
            }
        }
        OutputFormat::Json => {
            let json_output: Vec<_> = combined
                .iter()
                .map(|(name, maybe_ruby)| {
                    let (installed, path, symlink) = if let Some(ruby) = maybe_ruby {
                        (true, Some(ruby.executable_path()), ruby.symlink.clone())
                    } else {
                        (false, None, None)
                    };
                    let is_active = active.as_ref().is_some_and(|a| a.display_name() == *name);

                    serde_json::json!({
                        "name": name,
                        "installed": installed,
                        "active": is_active,
                        "path": path,
                        "symlink_target": symlink,
                    })
                })
                .collect();
            serde_json::to_writer_pretty(io::stdout(), &json_output)?;
        }
    }

    Ok(())
}

/// Prints a list of already installed rubies.
fn print_rubies(rubies: &[rv_ruby::Ruby], active: &Option<rv_ruby::Ruby>, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => {
            let width = rubies
                .iter()
                .map(|ruby| ruby.display_name().len())
                .max()
                .unwrap_or_default();

            for ruby in rubies {
                let entry = format_installed_ruby_entry(ruby, active, width);
                println!("{entry}");
            }
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(io::stdout(), rubies)?;
        }
    }
    Ok(())
}

/// Format a single installed Ruby entry for text output.
fn format_installed_ruby_entry(
    ruby: &rv_ruby::Ruby,
    active: &Option<rv_ruby::Ruby>,
    width: usize,
) -> String {
    let key = ruby.display_name();
    let path = ruby.executable_path();
    let marker = if active.as_ref().is_some_and(|a| a == ruby) {
        "*"
    } else {
        " "
    };

    if let Some(ref symlink_target) = ruby.symlink {
        format!(
            "{marker} {key:width$}    {} -> {}",
            path.cyan(),
            symlink_target.cyan()
        )
    } else {
        format!("{marker} {key:width$}    {}", path.cyan())
    }
}

/// Format a single Ruby entry for text output, indicating if it is installed or just available.
fn format_ruby_entry(
    name: &str,
    maybe_ruby: &Option<&rv_ruby::Ruby>,
    active: &Option<rv_ruby::Ruby>,
    width: usize,
) -> String {
    let marker = if active.as_ref().is_some_and(|a| a.display_name() == name) {
        "*"
    } else {
        " "
    };

    if let Some(ruby) = maybe_ruby {
        let path = ruby.executable_path();
        if let Some(ref symlink_target) = ruby.symlink {
            format!(
                "{marker} {name:width$} {} {} -> {}",
                "[installed]".green(),
                path.cyan(),
                symlink_target.cyan()
            )
        } else {
            format!(
                "{marker} {name:width$} {} {}",
                "[installed]".green(),
                path.cyan()
            )
        }
    } else {
        format!("{marker} {name:width$} {}", "[available]".dimmed())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use rv_cache::Cache;
    use tempfile::TempDir;

    fn test_config() -> Config {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = Utf8PathBuf::from(temp_dir.path().to_str().unwrap());
        let root = Utf8PathBuf::from("/tmp/rv_test_root");
        let rubies_dir = temp_path.join("rubies");
        let current_dir = temp_path.join("project");
        let current_exe = root.join("bin").join("rv");

        Config {
            ruby_dirs: vec![rubies_dir],
            gemfile: None,
            root,
            current_dir,
            project_dir: None,
            cache: Cache::temp().unwrap(),
            current_exe,
        }
    }

    #[tokio::test]
    async fn test_ruby_list_text_output() {
        let config = test_config();
        // Should not panic - basic smoke test
        list(&config, OutputFormat::Text, false).await.unwrap();
    }

    #[tokio::test]
    async fn test_ruby_list_json_output() {
        let config = test_config();
        // Should not panic - basic smoke test
        list(&config, OutputFormat::Json, false).await.unwrap();
    }
}
