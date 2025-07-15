use miette::Result;
use crate::config::Config;

/// Create a new gem
pub fn new_gem(config: &Config, name: &str, template: Option<&str>, skip_git: bool) -> Result<()> {
    println!("ruby_dirs configured: {}", config.ruby_dirs.len());
    println!("Creating new gem '{}'", name);

    if let Some(template_name) = template {
        println!("Using template: {}", template_name);
    }

    if skip_git {
        println!("Skipping git initialization");
    }

    println!("This command is not yet implemented.");
    println!("It will:");
    println!("  1. Generate gem directory structure");
    println!("  2. Create gemspec file with metadata");
    println!("  3. Set up basic lib/ and test/ directories");
    println!("  4. Initialize git repository (unless skipped)");
    println!("  5. Generate basic README and documentation");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_new_gem() {
        let config = Config::new();
        let result = new_gem(&config, "test_gem", None, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_gem_with_template() {
        let config = Config::new();
        let result = new_gem(&config, "test_gem", Some("minimal"), false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_gem_skip_git() {
        let config = Config::new();
        let result = new_gem(&config, "test_gem", None, true);
        assert!(result.is_ok());
    }
}
