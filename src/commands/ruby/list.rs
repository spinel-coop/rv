use crate::config::Config;
use miette::{IntoDiagnostic, Result};
use owo_colors::OwoColorize;
use std::env;
use std::fs;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Text,
    Json,
}

/// List the available Ruby installations
pub fn list_rubies(config: &Config, format: OutputFormat, _installed_only: bool) -> Result<()> {
    let rubies = config.rubies()?;

    if rubies.is_empty() {
        println!("No Ruby installations found.");
        println!("Try installing Ruby with 'rv ruby install' or check your configuration.");
        return Ok(());
    }

    match format {
        OutputFormat::Text => {
            for ruby in rubies {
                let is_active = is_active_ruby(&ruby)?;
                let marker = if is_active { "*" } else { " " };
                let name = ruby.display_name();
                let path = ruby.path.display();
                
                if is_active {
                    println!("{} {} {}", 
                        marker.green().bold(), 
                        name.green().bold(), 
                        path.to_string().dimmed()
                    );
                } else {
                    println!("{} {} {}", marker, name, path.to_string().dimmed());
                }
            }
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rubies).into_diagnostic()?;
            println!("{}", json);
        }
    }

    Ok(())
}

fn is_active_ruby(ruby: &crate::ruby::Ruby) -> Result<bool> {
    // Check .ruby-version file in current directory and parent directories
    if let Some(pinned_version) = find_pinned_version()? {
        if pinned_version == ruby.version || pinned_version == ruby.display_name() {
            return Ok(true);
        }
    }
    
    // Check if this Ruby is currently active in PATH
    if is_ruby_in_path(ruby)? {
        return Ok(true);
    }
    
    Ok(false)
}

/// Find pinned Ruby version by checking .ruby-version files
fn find_pinned_version() -> Result<Option<String>> {
    let mut current_dir = env::current_dir().into_diagnostic()?;
    
    loop {
        let ruby_version_file = current_dir.join(".ruby-version");
        if ruby_version_file.exists() {
            let version = fs::read_to_string(&ruby_version_file)
                .into_diagnostic()?
                .trim()
                .to_string();
            return Ok(Some(version));
        }
        
        match current_dir.parent() {
            Some(parent) => current_dir = parent.to_path_buf(),
            None => break,
        }
    }
    
    Ok(None)
}

/// Check if the given Ruby is the one currently active in PATH
fn is_ruby_in_path(ruby: &crate::ruby::Ruby) -> Result<bool> {
    // Try to find 'ruby' in PATH
    if let Ok(which_ruby) = which::which("ruby") {
        // Resolve symlinks and compare canonical paths
        let which_canonical = which_ruby.canonicalize().unwrap_or(which_ruby);
        let ruby_canonical = ruby.path.canonicalize().unwrap_or(ruby.path.clone());
        
        if which_canonical == ruby_canonical {
            return Ok(true);
        }
    }
    
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_ruby() -> crate::ruby::Ruby {
        crate::ruby::Ruby {
            key: "ruby-3.1.4-macos-aarch64".to_string(),
            version: "3.1.4".to_string(),
            version_parts: crate::ruby::VersionParts {
                major: 3,
                minor: 1,
                patch: 4,
                pre: None,
            },
            path: PathBuf::from("/opt/rubies/ruby-3.1.4/bin/ruby"),
            symlink: None,
            implementation: "ruby".to_string(),
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        }
    }

    #[test]
    fn test_find_pinned_version_in_current_dir() {
        let temp_dir = TempDir::new().unwrap();
        let ruby_version_file = temp_dir.path().join(".ruby-version");
        let mut file = std::fs::File::create(&ruby_version_file).unwrap();
        writeln!(file, "3.1.4").unwrap();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let result = find_pinned_version().unwrap();
        assert_eq!(result, Some("3.1.4".to_string()));

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_find_pinned_version_in_parent_dir() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        
        let ruby_version_file = temp_dir.path().join(".ruby-version");
        let mut file = std::fs::File::create(&ruby_version_file).unwrap();
        writeln!(file, "3.2.0").unwrap();

        // Change to subdirectory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&sub_dir).unwrap();

        let result = find_pinned_version().unwrap();
        assert_eq!(result, Some("3.2.0".to_string()));

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_find_pinned_version_not_found() {
        let temp_dir = TempDir::new().unwrap();
        
        // Change to temp directory (no .ruby-version file)
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let result = find_pinned_version().unwrap();
        assert_eq!(result, None);

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_is_active_ruby_with_pinned_version() {
        let temp_dir = TempDir::new().unwrap();
        let ruby_version_file = temp_dir.path().join(".ruby-version");
        let mut file = std::fs::File::create(&ruby_version_file).unwrap();
        writeln!(file, "3.1.4").unwrap();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let ruby = create_test_ruby();
        let result = is_active_ruby(&ruby).unwrap();
        assert!(result);

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_is_active_ruby_with_display_name_match() {
        let temp_dir = TempDir::new().unwrap();
        let ruby_version_file = temp_dir.path().join(".ruby-version");
        let mut file = std::fs::File::create(&ruby_version_file).unwrap();
        writeln!(file, "ruby-3.1.4").unwrap();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let ruby = create_test_ruby();
        let result = is_active_ruby(&ruby).unwrap();
        assert!(result);

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_is_active_ruby_no_match() {
        let temp_dir = TempDir::new().unwrap();
        let ruby_version_file = temp_dir.path().join(".ruby-version");
        let mut file = std::fs::File::create(&ruby_version_file).unwrap();
        writeln!(file, "3.2.0").unwrap();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let ruby = create_test_ruby();
        let result = is_active_ruby(&ruby).unwrap();
        assert!(!result);

        // Restore original directory
        env::set_current_dir(original_dir).unwrap();
    }
}
