use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;

use crate::config::Config;

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
            // Calculate the maximum width for the name column to align output
            // Using the same approach as uv with fold()
            let width = rubies.iter()
                .fold(0usize, |acc, ruby| acc.max(ruby.display_name().len()));
            
            for ruby in &rubies {
                let key = ruby.display_name();
                let path = ruby.executable_path();
                
                // Check if the path is a symlink and format accordingly
                // Following uv's exact pattern
                if let Some(ref symlink_target) = ruby.symlink {
                    println!(
                        "{key:width$}    {} -> {}",
                        path.display().to_string().cyan(),
                        symlink_target.as_str().cyan()
                    );
                } else {
                    println!(
                        "{key:width$}    {}",
                        path.display().to_string().cyan()
                    );
                }
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rubies).into_diagnostic()?;
            println!("{}", json);
        }
    }
    
    Ok(())
}