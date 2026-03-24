use crate::utils::is_homebrew_install;
use crate::{Commands, GlobalArgs, config::Config};
use axoupdater::AxoUpdater;
use rv_dirs::user_config_dir;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

const UPDATE_CHECK_FILENAME: &str = "rv_last_update_check";
const CHECK_INTERVAL_SECS: u64 = 60 * 60; // 1 hour

pub(crate) async fn update_if_needed(global_args: &GlobalArgs, command: &Commands) {
    if let Commands::Shell(_) = command {
        return;
    }

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

    // Check if installed via Homebrew
    if is_homebrew_install() {
        debug!("Detected Homebrew installation, skipping auto update.");
        return;
    }

    let config_dir = user_config_dir();

    fs_err::create_dir_all(config_dir.clone()).unwrap();

    let update_timestamp_file = config_dir.join(UPDATE_CHECK_FILENAME);

    let now_secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_secs(),
        Err(e) => {
            eprintln!("SystemTime before UNIX_EPOCH: {}", e);
            0
        }
    };

    if update_timestamp_file.exists() {
        let contents = fs::read_to_string(update_timestamp_file.clone())
            .expect("Can't read update timestamp file");

        let last_check = contents.trim().parse::<u64>().unwrap();

        if now_secs >= last_check && now_secs - last_check < CHECK_INTERVAL_SECS {
            let minutes = (now_secs - last_check) / 60;
            debug!(
                "Skipping update check: last checked {} minute(s) ago.",
                minutes
            );
            return;
        }
    }

    fs::write(&update_timestamp_file, now_secs.to_string()).unwrap();

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
                    debug!("Update failed: {:?}", e);
                }
            }
        }
    } else {
        debug!("No update needed");
    }
}
