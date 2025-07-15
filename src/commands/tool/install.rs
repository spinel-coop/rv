use miette::Result;
use crate::config::Config;

pub struct InstallToolArgs {
    pub tool: String,
    pub version: Option<String>,
}

/// Install a tool globally
pub fn install_tool(config: &Config, args: InstallToolArgs) -> Result<()> {
    println!("Using config with {} ruby directories", config.ruby_dirs.len());
    if let Some(ref v) = args.version {
        println!("Installing tool '{}' version '{}'", args.tool, v);
    } else {
        println!("Installing latest version of tool '{}'", args.tool);
    }
    println!("This command is not yet implemented.");
    println!("It will:");
    println!("  1. Resolve tool gem and version");
    println!("  2. Install with appropriate Ruby version");
    println!("  3. Create global executable wrapper");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_install_tool_with_version() {
        let config = Config::new();
        let args = InstallToolArgs {
            tool: "rails".to_string(),
            version: Some("7.0.0".to_string()),
        };
        
        let result = install_tool(&config, args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_install_tool_latest() {
        let config = Config::new();
        let args = InstallToolArgs {
            tool: "bundler".to_string(),
            version: None,
        };
        
        let result = install_tool(&config, args);
        assert!(result.is_ok());
    }
}
