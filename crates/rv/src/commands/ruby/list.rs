use std::io;

use owo_colors::OwoColorize;
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
    let active = config.project_ruby();

    if rubies.is_empty() {
        warn!("No Ruby installations found.");
        info!("Try installing Ruby with 'rv ruby install' or check your configuration.");
    }

    match format {
        OutputFormat::Text => {
            // Calculate the maximum width for the name column to align output
            let width = rubies
                .iter()
                .map(|ruby| ruby.display_name().len())
                .max()
                .unwrap_or_default();

            for ruby in &rubies {
                let entry = format_ruby_entry(ruby, &active, width);
                println!("{entry}");
            }
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(io::stdout(), &rubies)?;
        }
    }

    Ok(())
}

/// Format a single Ruby entry for text output
fn format_ruby_entry(ruby: &rv_ruby::Ruby, active: &Option<rv_ruby::Ruby>, width: usize) -> String {
    let key = ruby.display_name();
    let path = ruby.executable_path();

    // Check if this Ruby is active and add marker
    let marker = if active.as_ref().is_some_and(|a| a == ruby) {
        "*"
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
