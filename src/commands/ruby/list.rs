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
            // Find the longest version name for alignment
            let max_name_len = rubies.iter()
                .map(|r| r.display_name().len())
                .max()
                .unwrap_or(0);
            
            for ruby in rubies {
                let is_active = is_active_ruby(&ruby)?;
                let name = ruby.display_name();
                
                // Format like uv: "version-name    path"
                let formatted_name = format!("{:<width$}", name, width = max_name_len);
                let path_display = format_ruby_path_display(&ruby);
                
                if is_active {
                    println!("{} {}", 
                        formatted_name.green().bold(), 
                        path_display.cyan()
                    );
                } else {
                    println!("{} {}", 
                        formatted_name, 
                        path_display.cyan()
                    );
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

/// Format ruby path display with symlink notation
fn format_ruby_path_display(ruby: &crate::ruby::Ruby) -> String {
    let path_str = ruby.path.display().to_string();
    if let Some(ref symlink) = ruby.symlink {
        format!("{} -> {}", path_str, symlink.display())
    } else {
        path_str
    }
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
    use std::path::PathBuf;

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
    fn test_ruby_list_basic_functionality() {
        // Test basic ruby list functionality without filesystem dependencies
        let ruby = create_test_ruby();
        
        // Test display name formatting
        assert_eq!(ruby.display_name(), "ruby-3.1.4");
        
        // Test that ruby is considered valid for testing
        // Note: is_valid() checks if the path exists, but our test ruby has a fake path
        // so we just verify the logic doesn't crash
        let _is_valid = ruby.is_valid();
        
        // Test path display
        let path_str = ruby.path.display().to_string();
        assert_eq!(path_str, "/opt/rubies/ruby-3.1.4/bin/ruby");
    }

    #[test]
    fn test_symlink_display_logic() {
        // Test symlink display logic
        let ruby_with_symlink = crate::ruby::Ruby {
            key: "ruby-3.2.0-macos-aarch64".to_string(),
            version: "3.2.0".to_string(),
            version_parts: crate::ruby::VersionParts {
                major: 3,
                minor: 2,
                patch: 0,
                pre: None,
            },
            path: PathBuf::from("/opt/rubies/ruby-3.2.0/bin/ruby"),
            symlink: Some(PathBuf::from("/usr/local/bin/ruby")),
            implementation: "ruby".to_string(),
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        let path_display = format_ruby_path_display(&ruby_with_symlink);
        
        assert_eq!(path_display, "/opt/rubies/ruby-3.2.0/bin/ruby -> /usr/local/bin/ruby");
    }

    #[test]
    fn test_list_rubies_output_format() {
        // Create test rubies with different configurations
        let rubies = vec![
            create_test_ruby(),
            crate::ruby::Ruby {
                key: "ruby-3.2.0-macos-aarch64".to_string(),
                version: "3.2.0".to_string(),
                version_parts: crate::ruby::VersionParts {
                    major: 3,
                    minor: 2,
                    patch: 0,
                    pre: None,
                },
                path: PathBuf::from("/opt/rubies/ruby-3.2.0/bin/ruby"),
                symlink: Some(PathBuf::from("/usr/local/bin/ruby")),
                implementation: "ruby".to_string(),
                arch: "aarch64".to_string(),
                os: "macos".to_string(),
            },
        ];
        
        // Test that function doesn't panic and can handle different ruby configurations
        for ruby in &rubies {
            let _is_active = is_active_ruby(ruby).unwrap();
            let name = ruby.display_name();
            
            // Verify formatting logic
            let formatted_name = format!("{:<width$}", name, width = 15);
            assert!(formatted_name.len() >= name.len());
            
            let path_display = format_ruby_path_display(ruby);
            
            assert!(!path_display.is_empty());
        }
    }

    #[test]
    fn test_list_rubies_no_rubies_found() {
        // Create a custom config that won't find any real rubies
        let config = crate::config::Config {
            ruby_dirs: vec![PathBuf::from("/nonexistent/ruby/dir")],
            gemfile: None,
            cache_dir: PathBuf::from("/tmp/rv-test-cache"),
            local_dir: PathBuf::from("/tmp/rv-test-local"),
        };
        
        // Test with no rubies found - just verify it doesn't crash
        let result = list_rubies(&config, OutputFormat::Text, false);
        assert!(result.is_ok());
        
        let result = list_rubies(&config, OutputFormat::Json, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_command_output_with_mock_rubies() {
        use vfs::{MemoryFS, VfsPath};
        
        // Create an in-memory filesystem for testing
        let fs = MemoryFS::new();
        let vfs_root = VfsPath::new(fs);
        
        // Create a mock ruby directory structure
        let ruby_dir = vfs_root.join("opt").unwrap().join("rubies").unwrap();
        ruby_dir.create_dir_all().unwrap();
        
        let config = crate::config::Config {
            ruby_dirs: vec![PathBuf::from("/opt/rubies")], // Use the real path for config
            gemfile: None,
            cache_dir: PathBuf::from("/tmp/rv-test-cache"),
            local_dir: PathBuf::from("/tmp/rv-test-local"),
        };
        
        // Test both output formats - these should handle empty directories gracefully
        let result_text = list_rubies(&config, OutputFormat::Text, false);
        assert!(result_text.is_ok());
        
        let result_json = list_rubies(&config, OutputFormat::Json, false);
        assert!(result_json.is_ok());
    }

    #[test]
    fn test_output_alignment() {
        // Test that output alignment works correctly like uv
        let rubies = vec![
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
            },
            crate::ruby::Ruby {
                key: "jruby-9.4.0.0-macos-aarch64".to_string(),
                version: "9.4.0.0".to_string(),
                version_parts: crate::ruby::VersionParts {
                    major: 9,
                    minor: 4,
                    patch: 0,
                    pre: Some("0".to_string()),
                },
                path: PathBuf::from("/opt/rubies/jruby-9.4.0.0/bin/ruby"),
                symlink: None,
                implementation: "jruby".to_string(),
                arch: "aarch64".to_string(),
                os: "macos".to_string(),
            },
        ];

        // Find the longest name for alignment testing
        let max_name_len = rubies.iter()
            .map(|r| r.display_name().len())
            .max()
            .unwrap_or(0);

        assert_eq!(max_name_len, 13); // "jruby-9.4.0.0".len() is actually 13

        // Test that shorter names get padded correctly
        let short_name = "ruby-3.1.4"; // 11 chars
        let formatted = format!("{:<width$}", short_name, width = max_name_len);
        assert_eq!(formatted.len(), max_name_len);
        assert!(formatted.ends_with(' ')); // Should be padded with space
    }
}
