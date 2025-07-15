use miette::Result;
use crate::config::Config;

/// Remove a gem from the application
pub fn remove_gem(config: &Config, gem: &str) -> Result<()> {
    println!("Using config with {} ruby directories", config.ruby_dirs.len());
    println!("Removing gem '{}' from application", gem);

    println!("This command is not yet implemented.");
    println!("It will:");
    println!("  1. Remove gem from Gemfile");
    println!("  2. Run bundle install to update dependencies");
    println!("  3. Clean up unused dependencies");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_remove_gem_basic() {
        let config = Config::new();
        let result = remove_gem(&config, "rails");
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_gem_empty_name() {
        let config = Config::new();
        let result = remove_gem(&config, "");
        assert!(result.is_ok());
    }
}
