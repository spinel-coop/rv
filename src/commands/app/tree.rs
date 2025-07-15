use miette::Result;
use crate::config::Config;

/// Show dependency tree
pub fn show_tree(config: &Config, direct: bool) -> Result<()> {
    println!("Using config with {} ruby directories", config.ruby_dirs.len());
    println!("Showing dependency tree");

    if direct {
        println!("Showing only direct dependencies");
    } else {
        println!("Showing full dependency tree");
    }

    println!("This command is not yet implemented.");
    println!("It will:");
    println!("  1. Parse Gemfile.lock for dependency information");
    println!("  2. Build dependency graph");
    println!("  3. Display tree with version information");
    println!("  4. Highlight conflicts or outdated gems");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_show_tree_full() {
        let config = Config::new();
        let result = show_tree(&config, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_show_tree_direct_only() {
        let config = Config::new();
        let result = show_tree(&config, true);
        assert!(result.is_ok());
    }
}
