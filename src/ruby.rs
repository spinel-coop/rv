use std::path::{Path, PathBuf};
use std::fmt::{self, Display};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

#[serde_as]
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
    
    /// Ruby implementation
    #[serde_as(as = "DisplayFromStr")]
    pub implementation: RubyImplementation,
    
    /// System architecture (aarch64, x86_64, etc.)
    pub arch: String,
    
    /// Operating system (macos, linux, windows, etc.)
    pub os: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RubyImplementation {
    /// Standard Ruby (MRI/CRuby)
    Ruby,
    /// JRuby (Java implementation)
    #[serde(rename = "jruby")]
    JRuby,
    /// TruffleRuby (GraalVM implementation)
    #[serde(rename = "truffleruby")]
    TruffleRuby,
    /// mruby (minimal Ruby)
    #[serde(rename = "mruby")]
    MRuby,
    /// Artichoke Ruby (Rust implementation)
    #[serde(rename = "artichoke")]
    Artichoke,
    /// Unknown implementation with the original name
    #[serde(untagged)]
    Unknown(String),
}

impl RubyImplementation {
    /// Get the display name for this implementation
    pub fn name(&self) -> &str {
        match self {
            Self::Ruby => "ruby",
            Self::JRuby => "jruby",
            Self::TruffleRuby => "truffleruby",
            Self::MRuby => "mruby",
            Self::Artichoke => "artichoke",
            Self::Unknown(name) => name,
        }
    }
}

impl Display for RubyImplementation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl FromStr for RubyImplementation {
    type Err = std::convert::Infallible;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let implementation = match s {
            "ruby" => Self::Ruby,
            "jruby" => Self::JRuby,
            "truffleruby" => Self::TruffleRuby,
            "mruby" => Self::MRuby,
            "artichoke" => Self::Artichoke,
            _ => Self::Unknown(s.to_string()),
        };
        Ok(implementation)
    }
}

impl PartialOrd for RubyImplementation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RubyImplementation {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        
        // Get priority for each implementation
        let self_priority = match self {
            Self::Ruby => 0,           // Ruby always comes first
            Self::JRuby |
            Self::TruffleRuby |
            Self::MRuby |
            Self::Artichoke => 1,      // Known implementations second
            Self::Unknown(_) => 2,     // Unknown implementations last
        };
        
        let other_priority = match other {
            Self::Ruby => 0,
            Self::JRuby |
            Self::TruffleRuby |
            Self::MRuby |
            Self::Artichoke => 1,
            Self::Unknown(_) => 2,
        };
        
        // First compare by priority
        match self_priority.cmp(&other_priority) {
            Ordering::Equal => {
                // Same priority, sort alphabetically by name
                self.name().cmp(other.name())
            }
            other => other,
        }
    }
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
        let (implementation_name, version) = parse_ruby_dir_name(dir_name)?;
        let implementation = RubyImplementation::from_str(&implementation_name).unwrap();
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
        
        let key = format!("{}-{}-{}-{}", implementation.name(), version, os, arch);
        
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
        format!("{}-{}", self.implementation.name(), self.version)
    }
}

impl PartialOrd for Ruby {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ruby {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by implementation first
        match self.implementation.cmp(&other.implementation) {
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
///          "unknown-ruby-1.0.0" -> ("unknown-ruby", "1.0.0")
fn parse_ruby_dir_name(dir_name: &str) -> Result<(String, String), RubyError> {
    let parts: Vec<&str> = dir_name.splitn(2, '-').collect();
    if parts.len() != 2 {
        return Err(RubyError::InvalidDirectoryName(dir_name.to_string()));
    }
    
    let implementation = parts[0].to_string();
    let version = parts[1].to_string();
    
    // Accept any implementation name - the enum will handle unknown ones
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
        
        // Unknown implementations should be accepted now
        assert_eq!(
            parse_ruby_dir_name("unknown-1.0.0").unwrap(),
            ("unknown".to_string(), "1.0.0".to_string())
        );
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
            implementation: RubyImplementation::Ruby,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        let ruby2 = Ruby {
            key: "ruby-3.2.0-macos-aarch64".to_string(),
            version: "3.2.0".to_string(),
            version_parts: VersionParts { major: 3, minor: 2, patch: 0, pre: None },
            path: PathBuf::from("/opt/rubies/ruby-3.2.0/bin/ruby"),
            symlink: None,
            implementation: RubyImplementation::Ruby,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        let jruby = Ruby {
            key: "jruby-9.4.0.0-macos-aarch64".to_string(),
            version: "9.4.0.0".to_string(),
            version_parts: VersionParts { major: 9, minor: 4, patch: 0, pre: Some("0".to_string()) },
            path: PathBuf::from("/opt/rubies/jruby-9.4.0.0/bin/ruby"),
            symlink: None,
            implementation: RubyImplementation::JRuby,
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
    fn test_implementation_ordering() {
        let ruby = RubyImplementation::Ruby;
        let jruby = RubyImplementation::JRuby;
        let truffleruby = RubyImplementation::TruffleRuby;
        let unknown = RubyImplementation::Unknown("custom-ruby".to_string());
        
        // Ruby comes first
        assert!(ruby < jruby);
        assert!(ruby < truffleruby);
        assert!(ruby < unknown);
        
        // Known implementations come before unknown
        assert!(jruby < unknown);
        assert!(truffleruby < unknown);
        
        // Known implementations are sorted alphabetically
        assert!(jruby < truffleruby); // "jruby" < "truffleruby"
    }
    
    #[test]
    fn test_ruby_implementation_from_str() {
        assert_eq!(RubyImplementation::from_str("ruby").unwrap(), RubyImplementation::Ruby);
        assert_eq!(RubyImplementation::from_str("jruby").unwrap(), RubyImplementation::JRuby);
        assert_eq!(RubyImplementation::from_str("truffleruby").unwrap(), RubyImplementation::TruffleRuby);
        assert_eq!(RubyImplementation::from_str("mruby").unwrap(), RubyImplementation::MRuby);
        assert_eq!(RubyImplementation::from_str("artichoke").unwrap(), RubyImplementation::Artichoke);
        assert_eq!(RubyImplementation::from_str("custom-ruby").unwrap(), RubyImplementation::Unknown("custom-ruby".to_string()));
    }
    
    #[test]
    fn test_ruby_implementation_name() {
        assert_eq!(RubyImplementation::Ruby.name(), "ruby");
        assert_eq!(RubyImplementation::JRuby.name(), "jruby");
        assert_eq!(RubyImplementation::Unknown("custom-ruby".to_string()).name(), "custom-ruby");
    }
}