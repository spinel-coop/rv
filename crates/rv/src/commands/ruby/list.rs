use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;

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
        println!("No Ruby installations found.");
        println!("Try installing Ruby with 'rv ruby install' or check your configuration.");
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
                print_ruby_entry(ruby, &active_version, width);
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rubies).into_diagnostic()?;
            println!("{}", json);
        }
    }

    Ok(())
}

/// Print a single Ruby entry in the text format
fn print_ruby_entry(ruby: &crate::ruby::Ruby, active_version: &Option<String>, width: usize) {
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
        println!(
            "{marker} {key:width$}    {} -> {}",
            path.display().to_string().cyan(),
            symlink_target.as_str().cyan()
        );
    } else {
        println!(
            "{marker} {key:width$}    {}",
            path.display().to_string().cyan()
        );
    }
}
