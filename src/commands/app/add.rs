use miette::Result;
use crate::config::Config;

/// Add a gem to the application
pub fn add_gem(config: &Config, gem: &str, version: Option<&str>, dev: bool, test: bool) -> Result<()> {
    println!("Using config with {} ruby directories", config.ruby_dirs.len());
    println!("Adding gem '{}' to application", gem);

    if let Some(v) = version {
        println!("Version requirement: {}", v);
    }

    let group = if dev {
        "development"
    } else if test {
        "test"
    } else {
        "runtime"
    };
    println!("Adding to {} dependencies", group);

    println!("This command is not yet implemented.");
    println!("It will:");
    println!("  1. Add gem to Gemfile with appropriate group");
    println!("  2. Run bundle install to install the gem");
    println!("  3. Update lockfile and verify installation");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_add_gem_basic() {
        let config = Config::new();
        let result = add_gem(&config, "rails", None, false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_gem_with_version() {
        let config = Config::new();
        let result = add_gem(&config, "rails", Some("7.0"), false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_gem_dev_group() {
        let config = Config::new();
        let result = add_gem(&config, "rspec", None, true, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_gem_test_group() {
        let config = Config::new();
        let result = add_gem(&config, "factory_bot", None, false, true);
        assert!(result.is_ok());
    }
}
