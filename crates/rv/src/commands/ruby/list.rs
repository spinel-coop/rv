use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use tracing::{info, warn};

use crate::config::Config;
use crate::ruby::find_active_ruby_version;

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
    ruby: &crate::ruby::Ruby,
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
            symlink_target.as_str().cyan()
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
    use super::*;
    use tempfile::TempDir;
    use vfs::{AltrootFS, VfsPath};

    fn test_config() -> Config {
        let temp_dir = TempDir::new().unwrap();
        let physical_fs = vfs::PhysicalFS::new("/");
        let root = VfsPath::new(physical_fs);
        let temp_root = root
            .join(temp_dir.path().to_string_lossy().as_ref())
            .unwrap();
        let altroot_fs = AltrootFS::new(temp_root);
        let vfs_root = VfsPath::new(altroot_fs);

        Config {
            ruby_dirs: vec![vfs_root.join("rubies").unwrap()],
            gemfile: None,
            root,
            current_dir: vfs_root.join("project").unwrap(),
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
