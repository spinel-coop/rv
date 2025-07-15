use crate::config::Config;
use miette::{IntoDiagnostic, Result};
use std::fs;

/// Pin Ruby version for current project
pub fn pin_ruby(config: &Config, version: Option<&str>) -> Result<()> {
    println!("Config has {} ruby directories", config.ruby_dirs.len());
    
    match version {
        Some(v) => {
            println!("Pinning Ruby version '{}' for current project", v);

            // Write .ruby-version file
            fs::write(".ruby-version", format!("{}\n", v)).into_diagnostic()?;
            println!("Created .ruby-version file with version '{}'", v);
        }
        None => {
            // Show current pinned version
            match fs::read_to_string(".ruby-version") {
                Ok(content) => {
                    let version = content.trim();
                    if version.is_empty() {
                        println!("No Ruby version pinned for current project");
                    } else {
                        println!("Current pinned Ruby version: {}", version);
                    }
                }
                Err(_) => {
                    println!("No .ruby-version file found");
                    println!("Use 'rv ruby pin <version>' to pin a Ruby version");
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn test_config() -> Config {
        Config {
            ruby_dirs: vec![PathBuf::from("/test/ruby")],
            gemfile: None,
            cache_dir: PathBuf::from("/test/cache"),
            local_dir: PathBuf::from("/test/local"),
        }
    }

    #[test]
    fn test_pin_ruby_with_version() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let config = test_config();
        let result = pin_ruby(&config, Some("3.1.0"));
        
        std::env::set_current_dir(original_dir).unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn test_pin_ruby_show_current() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let config = test_config();
        let result = pin_ruby(&config, None);
        
        std::env::set_current_dir(original_dir).unwrap();
        assert!(result.is_ok());
    }
}
