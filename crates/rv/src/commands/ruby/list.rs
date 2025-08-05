use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use rv_ruby::find_active_ruby_version;
use tracing::{info, warn};

use crate::config::Config;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Text,
    Json,
}

pub fn list(config: &Config, format: OutputFormat, _installed_only: bool) -> Result<()> {
    let rubies = config.rubies()?;

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
            let json = serde_json::to_string_pretty(&rubies).into_diagnostic()?;
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
            path.display().to_string().cyan(),
            symlink_target.to_string_lossy().cyan()
        )
    } else {
        format!(
            "{marker} {key:width$}    {}",
            path.display().to_string().cyan()
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use tempfile::TempDir;

    fn test_config() -> Config {
        let temp_dir = TempDir::new().unwrap();
        let root = PathBuf::from("/tmp/rv_test_root");
        let rubies_dir = temp_dir.path().join("rubies");
        let current_dir = temp_dir.path().join("project");

        Config {
            ruby_dirs: vec![rubies_dir],
            gemfile: None,
            root,
            current_dir,
            project_dir: None,
        }
    }

    #[test]
    fn test_ruby_list_text_output() {
        let config = test_config();
        // Should not panic - basic smoke test
        list(&config, OutputFormat::Text, false).unwrap();
    }

    #[test]
    fn test_ruby_list_json_output() {
        let config = test_config();
        // Should not panic - basic smoke test
        list(&config, OutputFormat::Json, false).unwrap();
    }
}
