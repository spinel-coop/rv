use axoupdater::AxoUpdater;
use rv_dirs::user_state_dir;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};
use tracing::{debug, error};

const UPDATE_CHECK_FILENAME: &str = "rv_last_update_check";
const CHECK_INTERVAL_SECS: u64 = 60 * 60;

pub(crate) async fn check(update_mode: &str) {
    if update_mode == "none" || is_ci_env() || !is_time_to_check() {
        return;
    }

    run_check(update_mode).await;
}

pub(crate) async fn run_check(update_mode: &str) {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    if is_homebrew_install() {
        debug!("Detected Hombrew installation in update check.");
        let latest_release = match latest_homebrew_release("stable").await {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to fetch Homebrew version: {}", e);
                return;
            }
        };
        let latest_version = latest_release.version;

        if is_newer_version(&current_version, &latest_version) {
            if update_mode == "warning" {
                eprintln!(
                    "⚠️ There is a new version of `rv`: {}. Please update using `brew upgrade rv`.",
                    latest_version
                );
            } else {
                run_homebrew_upgrade();
            }
        }
    } else {
        let mut updater = AxoUpdater::new_for("rv");

        if updater.load_receipt().is_err()
            || !updater.check_receipt_is_for_this_executable().unwrap()
        {
            debug!("Update receipt is invalid or not for this executable, skipping update.");
            return;
        }

        if updater.is_update_needed().await.unwrap() {
            debug!("New rv version available.");
            if update_mode == "warning" {
                println!(
                    "⚠️ There is a new version of `rv`. Please update using `rv self update`."
                );
            } else {
                println!("⬆️ Installing new version of `rv`...");
                match updater.run().await {
                    Ok(r) => {
                        if let Some(result) = r {
                            println!("✅ `rv` {} installed!", result.new_version);
                        }
                        debug!("Successfully updated");
                    }
                    Err(e) => {
                        error!("Update failed: {:?}", e);
                    }
                }
            }
        } else {
            debug!("No update needed");
        }
    }
}

fn is_time_to_check() -> bool {
    let state_dir = user_state_dir("/".into());

    fs_err::create_dir_all(state_dir.clone()).unwrap();

    let update_timestamp_file = state_dir.join(UPDATE_CHECK_FILENAME);

    let now_secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_secs(),
        Err(e) => {
            error!("SystemTime before UNIX_EPOCH: {}", e);
            return false;
        }
    };

    if update_timestamp_file.exists() {
        let Ok(contents) = fs::read_to_string(update_timestamp_file.clone()) else {
            error!("Can't read update timestamp file.");
            return false;
        };

        let Some(last_check) = contents.trim().parse::<u64>().ok() else {
            error!("Failed to parse update timestamp");
            return false;
        };

        if now_secs >= last_check && now_secs - last_check < CHECK_INTERVAL_SECS {
            let minutes = (now_secs - last_check) / 60;
            debug!(
                "Skipping update check: last checked {} minute(s) ago.",
                minutes
            );
            return false;
        }
    }

    if let Err(e) = fs::write(&update_timestamp_file, now_secs.to_string()) {
        error!("Failed to write update timestamp file: {}", e);
        return false;
    }

    true
}

pub(crate) fn is_ci_env() -> bool {
    let ci_vars = ["CI", "CONTINUOUS_INTEGRATION"];

    for var in ci_vars.iter() {
        if env::var(var).is_ok() {
            return true;
        }
    }

    false
}

pub fn brew_prefix() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return None;
    }

    let brew_path = which::which("brew").ok()?;
    let output = Command::new(brew_path).arg("--prefix").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if prefix.is_empty() {
        return None;
    }

    Some(PathBuf::from(prefix))
}

pub fn is_homebrew_install() -> bool {
    if cfg!(target_family = "windows") {
        return false;
    }

    let exe_path = match env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let brew_prefix = match brew_prefix() {
        Some(p) => p,
        None => return false,
    };

    if !exe_path.starts_with(&brew_prefix) {
        return false;
    }

    let rel_path = match exe_path.strip_prefix(&brew_prefix) {
        Ok(p) => p,
        Err(_) => return false,
    };

    let has_bin = rel_path.components().any(|c| c.as_os_str() == "bin");

    let exe_name = match exe_path.file_name() {
        Some(n) => n,
        None => return false,
    };

    let ends_with_exe = matches!(rel_path.components().next_back(), Some(std::path::Component::Normal(n)) if n == exe_name);

    has_bin && ends_with_exe
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
}

#[derive(Debug, Deserialize)]
struct BrewResponse {
    versions: BrewVersions,
}

#[derive(Debug, Deserialize)]
struct BrewVersions {
    stable: String,
}

pub async fn latest_homebrew_release(_channel: &str) -> Result<Release, reqwest::Error> {
    let url = "https://formulae.brew.sh/api/formula/rv.json";
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?;
    let brew_resp: BrewResponse = resp.json().await?;
    let raw = brew_resp.versions.stable.trim();

    Ok(Release {
        version: raw.to_string(),
    })
}

fn normalize_version(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    s.split(&['-', '+'][..]).next().unwrap_or(s)
}

fn version_components(s: &str) -> Vec<u64> {
    normalize_version(s)
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0u64))
        .collect()
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ac = version_components(a);
    let mut bc = version_components(b);
    let max_len = std::cmp::max(ac.len(), bc.len());
    ac.resize(max_len, 0);
    bc.resize(max_len, 0);
    for i in 0..max_len {
        match ac[i].cmp(&bc[i]) {
            Ordering::Less => return Ordering::Less,
            Ordering::Greater => return Ordering::Greater,
            Ordering::Equal => continue,
        }
    }
    Ordering::Equal
}

pub fn is_newer_version(current: &str, latest: &str) -> bool {
    compare_versions(current, latest) == std::cmp::Ordering::Less
}

pub fn run_homebrew_upgrade() -> Result<(), Box<dyn std::error::Error>> {
    let brew_path = which::which("brew")?;

    // Update brew before upgrade so we can capture output
    let brew_update_output = Command::new(&brew_path)
        .arg("update")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !brew_update_output.status.success() {
        let stderr = String::from_utf8_lossy(&brew_update_output.stderr);
        error!("Brew update failed:\n{}", stderr);

        return Err(format!(
            "brew update failed with status: {}",
            brew_update_output.status
        )
        .into());
    }

    let rv_update_output = Command::new(&brew_path)
        .arg("upgrade")
        .arg("rv")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !rv_update_output.status.success() {
        let stderr = String::from_utf8_lossy(&rv_update_output.stderr);
        let stdout = String::from_utf8_lossy(&rv_update_output.stdout);

        error!("Brew upgrade failed:\n{}", stderr);
        error!("Stdout:\n{}", stdout);

        return Err(format!(
            "brew upgrade failed with status: {}",
            rv_update_output.status
        )
        .into());
    } else {
        debug!("Upgrade with brew success.")
    }

    Ok(())
}
