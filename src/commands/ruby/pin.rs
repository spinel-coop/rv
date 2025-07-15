use miette::{IntoDiagnostic, Result};
use std::fs;

/// Pin Ruby version for current project
pub fn pin_ruby(version: Option<&str>) -> Result<()> {
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
