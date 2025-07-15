use miette::Result;

/// Add a gem to the application
pub fn add_gem(gem: &str, version: Option<&str>, dev: bool, test: bool) -> Result<()> {
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
