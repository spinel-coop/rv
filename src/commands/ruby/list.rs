use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use tracing::info;

use crate::config::Config;
use crate::ruby::find_active_ruby_version;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Text,
    Json,
}

pub fn list_rubies(config: &Config, format: OutputFormat, _installed_only: bool) -> Result<()> {
    let rubies = config.rubies()?;

    if rubies.is_empty() {
        let msg1 = "No Ruby installations found.";
        let msg2 = "Try installing Ruby with 'rv ruby install' or check your configuration.";
        info!("{}", msg1);
        info!("{}", msg2);
        return Ok(());
    }

    match format {
        OutputFormat::Text => {
            // Find the active Ruby version for marking
            let active_version = find_active_ruby_version();

            // Calculate the maximum width for the name column to align output
            // Using the same approach as uv with fold()
            let width = rubies
                .iter()
                .fold(0usize, |acc, ruby| acc.max(ruby.display_name().len()));

            for ruby in &rubies {
                let entry = format_ruby_entry(ruby, &active_version, width);
                info!("{}", entry);
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rubies).into_diagnostic()?;
            info!("{}", json);
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
    // Following uv's exact pattern with active marker
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
    use insta::assert_snapshot;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tracing_test::traced_test;
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
            cache_dir: temp_dir.path().join("cache"),
            local_dir: temp_dir.path().join("local"),
            root,
        }
    }

    #[traced_test]
    #[test]
    fn test_ruby_list_text_output() {
        let config = test_config();
        list_rubies(&config, OutputFormat::Text, false).unwrap();

        // Capture all logs and snapshot test them
        let logs = logs_contain("No Ruby installations found.");
        assert!(logs);

        // TODO: Use insta to snapshot the actual output once we have proper output capture
    }

    #[traced_test]
    #[test]
    fn test_ruby_list_json_output() {
        let config = test_config();
        list_rubies(&config, OutputFormat::Json, false).unwrap();

        // Capture all logs and snapshot test them
        let logs = logs_contain("No Ruby installations found.");
        assert!(logs);

        // TODO: Use insta to snapshot the actual output once we have proper output capture
    }
}
