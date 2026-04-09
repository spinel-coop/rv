use axoupdater::AxoUpdater;
use rv_dirs::user_state_dir;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, thread};
use tracing::{debug, error};

const UPDATE_CHECK_FILENAME: &str = "rv_last_update_check";
const CHECK_INTERVAL_SECS: u64 = 60 * 60;

type Result<T> = miette::Result<T, Error>;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    AxoupdateError(#[from] axoupdater::AxoupdateError),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    WhichError(#[from] which::Error),

    #[error("brew command failed: {0}")]
    BrewFailed(String),

    #[error("update receipt invalid or not for this executable: {0}")]
    ReceiptInvalid(String),

    #[error("relaunch failed: {0}")]
    RelaunchFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    Installed(String),
    AlreadyUpToDate,
    UpdateAvailable(String),
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
}

pub(crate) async fn check(update_mode: &str) {
    if update_mode == "none" || is_ci_env() || !is_time_to_check() {
        return;
    }

    match run_update(update_mode).await {
        Ok(UpdateOutcome::Installed(v)) => {
            eprintln!("✅ New version of `rv` {} installed!", v);

            if let Err(e) = relaunch() {
                error!("Failed to relaunch updated rv: {}", e);
            }
        }
        Ok(UpdateOutcome::UpdateAvailable(latest)) => {
            if !latest.is_empty() {
                eprintln!(
                    "⚠️ There is a new version of `rv`: {}. Please update using `rv self update`.",
                    latest
                );
            } else {
                eprintln!(
                    "⚠️ There is a new version of `rv`. Please update using `rv self update`."
                );
            }
        }
        Ok(UpdateOutcome::AlreadyUpToDate) => {
            debug!("rv is already up to date.")
        }
        Err(e) => {
            error!("Self-update failed: {}", e);
        }
    }
}

pub(crate) async fn run_update(update_mode: &str) -> Result<UpdateOutcome> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    if is_homebrew_install() {
        debug!("Detected Homebrew installation in update check.");
        let latest_release = latest_homebrew_release("stable").await?;
        let latest_version = latest_release.version;

        if is_newer_version(&current_version, &latest_version) {
            if update_mode == "warning" {
                return Ok(UpdateOutcome::UpdateAvailable(latest_version));
            } else {
                run_homebrew_upgrade()?;
                return Ok(UpdateOutcome::Installed(latest_version));
            }
        } else {
            return Ok(UpdateOutcome::AlreadyUpToDate);
        }
    }

    let mut updater = AxoUpdater::new_for("rv");

    updater.load_receipt().map_err(Error::AxoupdateError)?;

    let is_for_executable = updater
        .check_receipt_is_for_this_executable()
        .map_err(Error::AxoupdateError)?;

    if !is_for_executable {
        return Err(Error::ReceiptInvalid(
            "receipt not for this executable".to_string(),
        ));
    }

    let needed = updater
        .is_update_needed()
        .await
        .map_err(Error::AxoupdateError)?;

    if needed {
        if update_mode == "warning" {
            return Ok(UpdateOutcome::UpdateAvailable(String::new()));
        }

        match updater.run().await.map_err(Error::AxoupdateError)? {
            Some(result) => Ok(UpdateOutcome::Installed(result.new_version.to_string())),
            None => Ok(UpdateOutcome::AlreadyUpToDate),
        }
    } else {
        Ok(UpdateOutcome::AlreadyUpToDate)
    }
}

fn is_time_to_check() -> bool {
    let state_dir = user_state_dir("/".into());

    if let Err(e) = fs_err::create_dir_all(state_dir.clone()) {
        error!("Failed to create state directory for update checks: {}", e);
        return false;
    }

    let update_timestamp_file = state_dir.join(UPDATE_CHECK_FILENAME);

    let now_secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_secs(),
        Err(e) => {
            error!("SystemTime before UNIX_EPOCH: {}", e);
            return false;
        }
    };

    if update_timestamp_file.exists() {
        let contents = match fs::read_to_string(update_timestamp_file.clone()) {
            Ok(c) => c,
            Err(e) => {
                error!("Can't read update timestamp file: {}", e);
                return false;
            }
        };

        let last_check = match contents.trim().parse::<u64>() {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse update timestamp: {}", e);
                return false;
            }
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

#[derive(Debug, Deserialize)]
struct BrewResponse {
    versions: BrewVersions,
}

#[derive(Debug, Deserialize)]
struct BrewVersions {
    stable: String,
}

pub async fn latest_homebrew_release(_channel: &str) -> Result<Release> {
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

pub fn run_homebrew_upgrade() -> Result<()> {
    let brew_path = which::which("brew")
        .map_err(|e| Error::BrewFailed(format!("Failed to locate 'brew' executable: {}", e)))?;

    let brew_update_output = Command::new(&brew_path)
        .arg("update")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| Error::BrewFailed(format!("Failed to run 'brew update': {}", e)))?;

    if !brew_update_output.status.success() {
        let stderr = String::from_utf8_lossy(&brew_update_output.stderr);

        return Err(Error::BrewFailed(format!(
            "brew update failed with status: {}. Error: {}",
            brew_update_output.status, stderr
        )));
    }

    let rv_update_output = Command::new(&brew_path)
        .arg("upgrade")
        .arg("rv")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| Error::BrewFailed(format!("Failed to run 'brew upgrade rv': {}", e)))?;

    if !rv_update_output.status.success() {
        let stderr = String::from_utf8_lossy(&rv_update_output.stderr);

        return Err(Error::BrewFailed(format!(
            "brew upgrade failed with status: {}. Error: {}",
            rv_update_output.status, stderr
        )));
    } else {
        debug!("The update with brew was successful.")
    }

    Ok(())
}

pub fn relaunch() -> Result<()> {
    thread::sleep(Duration::from_millis(400));

    #[cfg(target_os = "windows")]
    let exe_name = "rvw";
    #[cfg(not(target_os = "windows"))]
    let exe_name = "rv";

    let exe_path = which::which(exe_name).map_err(|e| {
        Error::RelaunchFailed(format!("Failed to locate executable '{}': {}", exe_name, e))
    })?;

    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut cmd = Command::new(&exe_path);
    cmd.args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    debug!(
        "Relaunching after update. Path: {:?}. Args: {:?}",
        exe_path, args
    );

    let mut child = cmd.spawn().map_err(|e| {
        Error::RelaunchFailed(format!("Failed to spawn '{}': {}", exe_path.display(), e))
    })?;

    let status = child.wait().map_err(|e| {
        Error::RelaunchFailed(format!(
            "Failed to wait for relaunched process '{}': {}",
            exe_path.display(),
            e
        ))
    })?;

    if let Some(code) = status.code() {
        std::process::exit(code);
    } else {
        std::process::exit(1);
    }
}
