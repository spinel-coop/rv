use miette::Result;
use crate::config::Config;

pub struct UninstallToolArgs {
    pub tool: String,
}

/// Uninstall a global tool
pub fn uninstall_tool(config: &Config, args: UninstallToolArgs) -> Result<()> {
    println!("Uninstalling tool '{}'", args.tool);
    println!("Using config with {} ruby directories", config.ruby_dirs.len());
    println!("This command is not yet implemented.");
    println!("It will:");
    println!("  1. Find installed tool location");
    println!("  2. Remove tool executables and data");
    println!("  3. Clean up associated Ruby installation if no longer needed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_uninstall_tool_basic() {
        let config = Config::new();
        let args = UninstallToolArgs {
            tool: "rails".to_string(),
        };
        
        let result = uninstall_tool(&config, args);
        assert!(result.is_ok());
    }
}
