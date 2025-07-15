use crate::config::Config;
use miette::Result;

/// Uninstall a Ruby version
pub fn uninstall_ruby(config: &Config, version: &str) -> Result<()> {
    println!("Config has {} ruby directories", config.ruby_dirs.len());
    
    println!("Uninstalling Ruby version '{}'", version);

    println!("This command is not yet implemented.");
    println!("It will:");
    println!("  1. Find the specified Ruby installation");
    println!("  2. Check if Ruby is currently in use");
    println!("  3. Remove Ruby directory and associated files");
    println!("  4. Clean up symlinks and PATH entries");
    println!("  5. Update shell configuration if needed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> Config {
        Config {
            ruby_dirs: vec![PathBuf::from("/test/ruby")],
            gemfile: None,
            cache_dir: PathBuf::from("/test/cache"),
            local_dir: PathBuf::from("/test/local"),
        }
    }

    #[test]
    fn test_uninstall_ruby() {
        let config = test_config();
        let result = uninstall_ruby(&config, "3.1.0");
        assert!(result.is_ok());
    }

    #[test]
    fn test_uninstall_ruby_different_version() {
        let config = test_config();
        let result = uninstall_ruby(&config, "2.7.5");
        assert!(result.is_ok());
    }
}
