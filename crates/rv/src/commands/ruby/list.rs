use owo_colors::OwoColorize;
use rv_ruby::find_active_ruby_version;
use tracing::{info, warn};

use crate::config::Config;

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
}

type Result<T> = miette::Result<T, Error>;

pub fn list(config: &Config, format: OutputFormat, _installed_only: bool) -> Result<()> {
    let rubies = config.rubies();

    if rubies.is_empty() {
        warn!("No Ruby installations found.");
        info!("Try installing Ruby with 'rv ruby install' or check your configuration.");
    }

    match format {
        OutputFormat::Text => {
            // Find the active Ruby version for marking
            let active_version = find_active_ruby_version();

            // Calculate the maximum width for the name column to align output
            let width = rubies
                .iter()
                .fold(0usize, |acc, ruby| acc.max(ruby.display_name().len()));

            for ruby in &rubies {
                let entry = format_ruby_entry(ruby, &active_version, width);
                println!("{entry}");
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rubies)?;
            println!("{json}");
        }
    }

    Ok(())
}

/// Format a single Ruby entry for text output
fn format_ruby_entry(
    ruby: &rv_ruby::Ruby,
    active_version: &Option<String>,
    width: usize,
) -> String {
    let key = ruby.display_name();
    let path = ruby.executable_path();

    // Check if this Ruby is active and add marker
    let marker = if let Some(active) = active_version {
        if ruby.is_active(active) { "*" } else { " " }
    } else {
        " "
    };

    // Check if the path is a symlink and format accordingly
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

        Config {
            ruby_dirs: vec![rubies_dir],
            gemfile: None,
            root,
            current_dir,
            project_dir: None,
            cache: Cache::temp().unwrap(),
        }
    }

    #[tokio::test]
    async fn test_ruby_list_text_output() {
        let config = test_config();
        // Should not panic - basic smoke test
        list(&config, OutputFormat::Text, false).unwrap();
    }

    #[tokio::test]
    async fn test_ruby_list_json_output() {
        let config = test_config();
        // Should not panic - basic smoke test
        list(&config, OutputFormat::Json, false).unwrap();
    }
}
