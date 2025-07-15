use miette::Result;

pub struct InstallToolArgs {
    pub tool: String,
    pub version: Option<String>,
}

/// Install a tool globally
pub fn install_tool(args: InstallToolArgs) -> Result<()> {
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
