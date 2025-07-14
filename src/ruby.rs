use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ruby {
    /// Unique identifier for this Ruby installation
    pub key: String,
    
    /// Ruby version (e.g., "3.1.4", "9.4.0.0")
    pub version: String,
    
    /// Parsed version components
    pub version_parts: VersionParts,
    
    /// Full path to the Ruby executable
    pub path: PathBuf,
    
    /// Symlink target if this Ruby is a symlink
    pub symlink: Option<PathBuf>,
    
    /// Ruby implementation (ruby, jruby, truffleruby, etc.)
    pub implementation: String,
    
    /// System architecture (aarch64, x86_64, etc.)
    pub arch: String,
    
    /// Operating system (macos, linux, windows, etc.)
    pub os: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionParts {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre: Option<String>,
}

impl Ruby {
    /// Create a new Ruby instance from a directory path
    pub fn from_dir<P: AsRef<Path>>(dir_path: P) -> Result<Self, RubyError> {
        let dir_path = dir_path.as_ref();
        let dir_name = dir_path.file_name()
            .and_then(|n| n.to_str())
            .ok_or(RubyError::InvalidPath)?;
        
        // Parse directory name (e.g., "ruby-3.1.4", "jruby-9.4.0.0")
        let (implementation, version) = parse_ruby_dir_name(dir_name)?;
        let version_parts = parse_version(&version)?;
        
        // Check for Ruby executable
        let ruby_bin = dir_path.join("bin").join("ruby");
        if !ruby_bin.exists() {
            return Err(RubyError::NoRubyExecutable);
        }
        
        // Check if it's a symlink
        let symlink = if ruby_bin.is_symlink() {
            std::fs::read_link(&ruby_bin).ok()
        } else {
            None
        };
        
        // Generate unique key
        let arch = std::env::consts::ARCH;
        let os = match std::env::consts::OS {
            "macos" => "macos",
            "linux" => "linux", 
            "windows" => "windows",
            other => other,
        };
        
        let key = format!("{}-{}-{}-{}", implementation, version, os, arch);
        
        Ok(Ruby {
            key,
            version,
            version_parts,
            path: ruby_bin,
            symlink,
            implementation,
            arch: arch.to_string(),
            os: os.to_string(),
        })
    }
    
    /// Check if this Ruby installation is valid
    pub fn is_valid(&self) -> bool {
        self.path.exists() && self.path.is_file()
    }
    
    /// Get display name for this Ruby
    pub fn display_name(&self) -> String {
        format!("{}-{}", self.implementation, self.version)
    }
}

impl PartialOrd for Ruby {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ruby {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort "ruby" first, then other implementations alphabetically
        let self_priority = implementation_priority(&self.implementation);
        let other_priority = implementation_priority(&other.implementation);
        
        match self_priority.cmp(&other_priority) {
            std::cmp::Ordering::Equal => {
                // Same implementation, compare versions: major.minor.patch
                match self.version_parts.major.cmp(&other.version_parts.major) {
                    std::cmp::Ordering::Equal => {
                        match self.version_parts.minor.cmp(&other.version_parts.minor) {
                            std::cmp::Ordering::Equal => {
                                self.version_parts.patch.cmp(&other.version_parts.patch)
                            }
                            other => other,
                        }
                    }
                    other => other,
                }
            }
            other => other,
        }
    }
}

/// Get sorting priority for Ruby implementations
/// Returns (priority, name) where lower priority sorts first
fn implementation_priority(implementation: &str) -> (u8, &str) {
    match implementation {
        "ruby" => (0, implementation),  // Ruby always comes first
        _ => (1, implementation),       // Others sorted alphabetically
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RubyError {
    #[error("Invalid path")]
    InvalidPath,
    #[error("No ruby executable found in bin/ directory")]
    NoRubyExecutable,
    #[error("Failed to parse Ruby directory name: {0}")]
    InvalidDirectoryName(String),
    #[error("Failed to parse version: {0}")]
    InvalidVersion(String),
}

/// Parse Ruby directory name into implementation and version
/// Examples: "ruby-3.1.4" -> ("ruby", "3.1.4")
///          "jruby-9.4.0.0" -> ("jruby", "9.4.0.0")
fn parse_ruby_dir_name(dir_name: &str) -> Result<(String, String), RubyError> {
    let parts: Vec<&str> = dir_name.splitn(2, '-').collect();
    if parts.len() != 2 {
        return Err(RubyError::InvalidDirectoryName(dir_name.to_string()));
    }
    
    let implementation = parts[0].to_string();
    let version = parts[1].to_string();
    
    // Validate known Ruby implementations
    match implementation.as_str() {
        "ruby" | "jruby" | "truffleruby" | "mruby" | "artichoke" => {},
        _ => return Err(RubyError::InvalidDirectoryName(format!("Unknown Ruby implementation: {}", implementation))),
    }
    
    Ok((implementation, version))
}

/// Parse version string into VersionParts
/// Examples: "3.1.4" -> VersionParts { major: 3, minor: 1, patch: 4, pre: None }
///          "9.4.0.0" -> VersionParts { major: 9, minor: 4, patch: 0, pre: Some("0") }
fn parse_version(version: &str) -> Result<VersionParts, RubyError> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 3 {
        return Err(RubyError::InvalidVersion(version.to_string()));
    }
    
    let major = parts[0].parse()
        .map_err(|_| RubyError::InvalidVersion(version.to_string()))?;
    let minor = parts[1].parse()
        .map_err(|_| RubyError::InvalidVersion(version.to_string()))?;
    let patch = parts[2].parse()
        .map_err(|_| RubyError::InvalidVersion(version.to_string()))?;
    
    // Handle additional version parts (like JRuby's 4th component)
    let pre = if parts.len() > 3 {
        Some(parts[3..].join("."))
    } else {
        None
    };
    
    Ok(VersionParts {
        major,
        minor,
        patch,
        pre,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_ruby_dir_name() {
        assert_eq!(
            parse_ruby_dir_name("ruby-3.1.4").unwrap(),
            ("ruby".to_string(), "3.1.4".to_string())
        );
        
        assert_eq!(
            parse_ruby_dir_name("jruby-9.4.0.0").unwrap(),
            ("jruby".to_string(), "9.4.0.0".to_string())
        );
        
        assert!(parse_ruby_dir_name("invalid").is_err());
        assert!(parse_ruby_dir_name("unknown-1.0.0").is_err());
    }
    
    #[test]
    fn test_parse_version() {
        let version = parse_version("3.1.4").unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 4);
        assert_eq!(version.pre, None);
        
        let version = parse_version("9.4.0.0").unwrap();
        assert_eq!(version.major, 9);
        assert_eq!(version.minor, 4);
        assert_eq!(version.patch, 0);
        assert_eq!(version.pre, Some("0".to_string()));
        
        assert!(parse_version("1.2").is_err());
        assert!(parse_version("invalid").is_err());
    }
    
    #[test]
    fn test_ruby_ordering() {
        let ruby1 = Ruby {
            key: "ruby-3.1.4-macos-aarch64".to_string(),
            version: "3.1.4".to_string(),
            version_parts: VersionParts { major: 3, minor: 1, patch: 4, pre: None },
            path: PathBuf::from("/opt/rubies/ruby-3.1.4/bin/ruby"),
            symlink: None,
            implementation: "ruby".to_string(),
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        let ruby2 = Ruby {
            key: "ruby-3.2.0-macos-aarch64".to_string(),
            version: "3.2.0".to_string(),
            version_parts: VersionParts { major: 3, minor: 2, patch: 0, pre: None },
            path: PathBuf::from("/opt/rubies/ruby-3.2.0/bin/ruby"),
            symlink: None,
            implementation: "ruby".to_string(),
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        let jruby = Ruby {
            key: "jruby-9.4.0.0-macos-aarch64".to_string(),
            version: "9.4.0.0".to_string(),
            version_parts: VersionParts { major: 9, minor: 4, patch: 0, pre: Some("0".to_string()) },
            path: PathBuf::from("/opt/rubies/jruby-9.4.0.0/bin/ruby"),
            symlink: None,
            implementation: "jruby".to_string(),
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        // Test version ordering within same implementation
        assert!(ruby1 < ruby2);
        
        // Test implementation priority: ruby comes before jruby
        assert!(ruby1 < jruby);
        assert!(ruby2 < jruby);
    }
    
    #[test]
    fn test_implementation_priority() {
        assert_eq!(implementation_priority("ruby"), (0, "ruby"));
        assert_eq!(implementation_priority("jruby"), (1, "jruby"));
        assert_eq!(implementation_priority("truffleruby"), (1, "truffleruby"));
    }
}