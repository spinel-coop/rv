use miette::{IntoDiagnostic, Result};
use crate::config::Config;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Text,
    Json,
}

/// List the available Ruby installations
pub fn list_rubies(config: &Config, format: OutputFormat, _installed_only: bool) -> Result<()> {
    let rubies = config.rubies()?;
    
    if rubies.is_empty() {
        println!("No Ruby installations found.");
        println!("Try installing Ruby with 'rv ruby install' or check your configuration.");
        return Ok(());
    }
    
    match format {
        OutputFormat::Text => {
            for ruby in rubies {
                let marker = if is_active_ruby(&ruby)? { "*" } else { " " };
                println!("{} {} {}", marker, ruby.display_name(), ruby.path.display());
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rubies).into_diagnostic()?;
            println!("{}", json);
        }
    }
    
    Ok(())
}

fn is_active_ruby(_ruby: &crate::ruby::Ruby) -> Result<bool> {
    // TODO: Implement active Ruby detection
    // 1. Check .ruby-version file in current directory
    // 2. Check global configuration
    // 3. Check PATH for currently active Ruby
    Ok(false)
}