use std::time::{Duration, SystemTime};

use anstream::eprintln;
use chrono::{Datelike, NaiveDate, Utc};
use fs_err as fs;
use owo_colors::OwoColorize;
use rv_cache::{Cache, CacheBucket};
use rv_client::http_client::rv_http_client;
use rv_ruby::version::RubyVersion;
use serde::{Deserialize, Serialize};
use tracing::debug;

const EOL_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60 * 7); // 7 days
const EOL_CACHE_FILE: &str = "ruby-eol.json";

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
}

type Result<T> = miette::Result<T, Error>;

#[derive(Deserialize)]
struct EndOfLifeResponse {
    result: EndOfLifeResult,
}

#[derive(Deserialize)]
struct EndOfLifeResult {
    releases: Vec<EndOfLifeRelease>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndOfLifeRelease {
    pub name: String,
    pub is_eol: bool,
    pub eol_from: String,
    pub release_date: String,
}

#[derive(Serialize, Deserialize)]
struct CachedEolData {
    expires_at: SystemTime,
    releases: Vec<EndOfLifeRelease>,
}

async fn fetch_end_of_life_information() -> Result<Vec<EndOfLifeRelease>> {
    let url = "https://endoflife.date/api/v1/products/ruby";
    let client = rv_http_client("rv_eol")?;
    let resp = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?;
    let eol_resp: EndOfLifeResponse = resp.json().await?;
    Ok(eol_resp.result.releases)
}

pub async fn get_cached_or_fetch(cache: &Cache) -> Result<Vec<EndOfLifeRelease>> {
    let cache_entry = cache.entry(CacheBucket::General, "eol", EOL_CACHE_FILE);

    if let Ok(content) = fs::read_to_string(cache_entry.path()) {
        match serde_json::from_str::<CachedEolData>(&content) {
            Ok(cached) if SystemTime::now() < cached.expires_at => {
                debug!("Using cached Ruby EOL data.");
                return Ok(cached.releases);
            }
            Ok(_) => debug!("Ruby EOL cache is stale, re-fetching."),
            Err(e) => debug!("Ruby EOL cache is corrupt, re-fetching: {e}"),
        }
    }

    let releases = fetch_end_of_life_information().await?;

    let cached = CachedEolData {
        expires_at: SystemTime::now() + EOL_CACHE_TTL,
        releases,
    };
    if let Some(parent) = cache_entry.path().parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(&cached) {
        let _ = fs::write(cache_entry.path(), json);
    }

    Ok(cached.releases)
}

pub async fn eol_information_for(
    version: &RubyVersion,
    cache: &Cache,
) -> Result<Option<EndOfLifeRelease>> {
    let releases = get_cached_or_fetch(cache).await?;
    let minor_key = format!("{}.{}", version.major, version.minor);
    Ok(releases.into_iter().find(|r| r.name == minor_key))
}

pub async fn eol_warning(version: &RubyVersion, cache: &Cache) {
    match eol_information_for(version, cache).await {
        Ok(Some(release)) if release.is_eol => {
            eprintln!(
                "{} Ruby {} ({}) reached End of Life on {}. \
                 Consider upgrading to a supported version.",
                "⚠️  Warning:".yellow().bold(),
                version.to_string().cyan().bold(),
                release.name,
                release.eol_from.red().bold(),
            );
        }
        Ok(Some(_)) => {
            // Version is still supported – no warning needed.
        }
        Ok(None) => {
            debug!("No EOL information found for Ruby {version}.");
        }
        Err(e) => {
            debug!("Failed to fetch EOL information for Ruby {version}: {e}");
        }
    }
}

pub fn format_eol_status_opt(release_opt: Option<&EndOfLifeRelease>) -> String {
    if let Some(release) = release_opt {
        if release.is_eol {
            return release.eol_from.clone();
        }

        if let Ok(eol_date) = NaiveDate::parse_from_str(&release.eol_from, "%Y-%m-%d") {
            let now = Utc::now().date_naive();
            let years = eol_date.year() - now.year();
            let months = (years * 12) + (eol_date.month() as i32 - now.month() as i32);
            if months <= 0 {
                let days = (eol_date - now).num_days();
                if days <= 1 {
                    return "in 1 day".to_string();
                }
                return format!("in {} days", days);
            }
            if months < 12 {
                if months == 1 {
                    return "in 1 month".to_string();
                }
                return format!("in {} months", months);
            }
            let years = months / 12;
            if years == 1 {
                return "in 1 year".to_string();
            }
            return format!("in {} years", years);
        }

        return release.eol_from.clone();
    }
    String::new()
}
