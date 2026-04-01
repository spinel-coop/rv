use crate::{Commands, GlobalArgs, config::Config};
use axoupdater::AxoUpdater;
use rv_dirs::user_state_dir;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};
use tracing::{debug, error};

const UPDATE_CHECK_FILENAME: &str = "rv_last_update_check";
const CHECK_INTERVAL_SECS: u64 = 60 * 60; // 1 hour

pub(crate) async fn update_if_needed(global_args: &GlobalArgs) {
    let config_result = Config::with_settings(global_args, None);
    let config = match &config_result {
        Ok(cfg) => cfg,
        Err(e) => {
            debug!("Error loading settings: {:?}", e);
            return;
        }
    };

    if config.rv_settings.update_mode == "none" {
        return;
    }

    let state_dir = user_state_dir("/".into());

    fs_err::create_dir_all(state_dir.clone()).unwrap();

    let update_timestamp_file = state_dir.join(UPDATE_CHECK_FILENAME);

    let now_secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_secs(),
        Err(e) => {
            error!("SystemTime before UNIX_EPOCH: {}", e);
            return;
        }
    };

    if update_timestamp_file.exists() {
        let Ok(contents) = fs::read_to_string(update_timestamp_file.clone()) else {
            error!("Can't read update timestamp file.");
            return;
        };

        let Some(last_check) = contents.trim().parse::<u64>().ok() else {
            error!("Failed to parse update timestamp");
            return;
        };

        if now_secs >= last_check && now_secs - last_check < CHECK_INTERVAL_SECS {
            let minutes = (now_secs - last_check) / 60;
            debug!(
                "Skipping update check: last checked {} minute(s) ago.",
                minutes
            );
            return;
        }
    }

    // Check if installed via Homebrew
    if is_homebrew_install() {
        debug!("Detected Homebrew installation, skipping auto update.");
        return;
    }

    if let Err(e) = fs::write(&update_timestamp_file, now_secs.to_string()) {
        error!("Failed to write update timestamp file: {}", e);
        return;
    }

    let mut updater = AxoUpdater::new_for("rv");

    if updater.load_receipt().is_err() || !updater.check_receipt_is_for_this_executable().unwrap() {
        debug!("Update receipt is invalid or not for this executable, skipping update.");
        return;
    }

    if updater.is_update_needed().await.unwrap() {
        debug!("Update needed.");
        if config.rv_settings.update_mode == "warning" {
            println!("⚠️ There is a new version of `rv`. Please update using `rv self update`.");
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

pub(crate) fn allowed_to_autoupdate(command: &Commands) -> bool {
    if let Commands::Shell(_) = command {
        return false;
    }

    let ci_vars = ["CI", "CONTINUOUS_INTEGRATION"];

    for var in ci_vars.iter() {
        if env::var(var).is_ok() {
            return false;
        }
    }

    true
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
