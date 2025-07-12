use std::path::PathBuf;
use std::fmt::{self, Display};
use std::str::FromStr;
use serde::{Deserialize, Serialize, Serializer};
use serde_with::{serde_as, DisplayFromStr};
use vfs::VfsPath;

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ruby {
    /// Unique identifier for this Ruby installation
    pub key: String,
    
    /// Ruby version (e.g., "3.1.4", "9.4.0.0")
    pub version: String,
    
    /// Parsed version components
    #[serde(skip)]
    pub version_parts: VersionParts,
    
    /// VFS path to the Ruby installation directory
    #[serde(serialize_with = "serialize_vfs_path")]
    pub path: VfsPath,
    
    /// Symlink target if this Ruby is a symlink
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "serialize_optional_vfs_path")]
    pub symlink: Option<VfsPath>,
    
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
    /// Create a new Ruby instance from a VFS directory path
    pub fn from_dir(vfs_dir: VfsPath) -> Result<Self, RubyError> {
        let dir_name = vfs_dir.filename();
        if dir_name.is_empty() {
            return Err(RubyError::InvalidPath { 
                path: vfs_dir.as_str().to_string() 
            });
        }
        
        // Parse directory name (e.g., "ruby-3.1.4", "jruby-9.4.0.0")
        let (implementation_name, version) = parse_ruby_dir_name(&dir_name)?;
        let implementation = RubyImplementation::from_str(&implementation_name).unwrap();
        let version_parts = parse_version(&version)?;
        
        // Check for Ruby executable
        let ruby_bin = vfs_dir.join("bin")?.join("ruby")?;
        if !ruby_bin.exists()? {
            return Err(RubyError::NoRubyExecutable);
        }
        
        let symlink = find_symlink_target(&ruby_bin);
        
        // Generate unique key
        let arch = std::env::consts::ARCH;
        let os = std::env::consts::OS;
        
        let key = format!("{}-{}-{}-{}", implementation.name(), version, os, arch);
        
        Ok(Ruby {
            key,
            version,
            version_parts,
            path: vfs_dir,
            symlink,
            implementation,
            arch: arch.to_string(),
            os: os.to_string(),
        })
    }
    
    /// Check if this Ruby installation is valid
    pub fn is_valid(&self) -> bool {
        // Use VFS to check validity
        let ruby_bin = self.path.join("bin").and_then(|bin| bin.join("ruby"));
        match ruby_bin {
            Ok(bin_path) => bin_path.exists().unwrap_or(false),
            Err(_) => false,
        }
    }
    
    /// Get display name for this Ruby
    pub fn display_name(&self) -> String {
        format!("{}-{}", self.implementation.name(), self.version)
    }
    
    /// Get the path to the Ruby executable for display purposes
    pub fn executable_path(&self) -> PathBuf {
        PathBuf::from(self.path.join("bin").unwrap().join("ruby").unwrap().as_str())
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
                // Same implementation, compare versions: major.minor.patch (descending order)
                match other.version_parts.major.cmp(&self.version_parts.major) {
                    std::cmp::Ordering::Equal => {
                        match other.version_parts.minor.cmp(&self.version_parts.minor) {
                            std::cmp::Ordering::Equal => {
                                other.version_parts.patch.cmp(&self.version_parts.patch)
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
    #[error("Invalid path: {path}")]
    InvalidPath { path: String },
    #[error("No ruby executable found in bin/ directory")]
    NoRubyExecutable,
    #[error("Failed to parse Ruby directory name: {0}")]
    InvalidDirectoryName(String),
    #[error("Failed to parse version: {0}")]
    InvalidVersion(String),
    #[error("VFS error: {0}")]
    VfsError(#[from] vfs::VfsError),
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
        use vfs::{PhysicalFS, VfsPath};
        
        // Create a dummy VFS path for testing
        let fs = PhysicalFS::new("/");
        let vfs_root = VfsPath::new(fs);
        let dummy_vfs_path = vfs_root.join("tmp").unwrap();
        
        let ruby1 = Ruby {
            key: "ruby-3.1.4-macos-aarch64".to_string(),
            version: "3.1.4".to_string(),
            version_parts: VersionParts { major: 3, minor: 1, patch: 4, pre: None },
            path: dummy_vfs_path.clone(),
            symlink: None,
            implementation: RubyImplementation::Ruby,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        let ruby2 = Ruby {
            key: "ruby-3.2.0-macos-aarch64".to_string(),
            version: "3.2.0".to_string(),
            version_parts: VersionParts { major: 3, minor: 2, patch: 0, pre: None },
            path: dummy_vfs_path.clone(),
            symlink: None,
            implementation: RubyImplementation::Ruby,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        let jruby = Ruby {
            key: "jruby-9.4.0.0-macos-aarch64".to_string(),
            version: "9.4.0.0".to_string(),
            version_parts: VersionParts { major: 9, minor: 4, patch: 0, pre: Some("0".to_string()) },
            path: dummy_vfs_path,
            symlink: None,
            implementation: RubyImplementation::JRuby,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
        };
        
        // Test version ordering within same implementation (higher versions first)
        assert!(ruby2 < ruby1); // 3.2.0 comes before 3.1.4
        
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

/// Custom serializer for VfsPath that serializes as the display string
fn serialize_vfs_path<S>(path: &VfsPath, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(path.as_str())
}

/// Custom serializer for Option<VfsPath> that serializes as the display string
fn serialize_optional_vfs_path<S>(path: &Option<VfsPath>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match path {
        Some(p) => serializer.serialize_str(p.as_str()),
        None => serializer.serialize_none(),
    }
}

/// Find symlink target for a VFS path, if it exists
fn find_symlink_target(vfs_path: &VfsPath) -> Option<VfsPath> {
    if let Ok(metadata) = vfs_path.metadata() {
        if metadata.file_type == vfs::VfsFileType::File {
            // For VFS, we need to check if it's a symlink using the underlying filesystem
            // Since VFS doesn't expose symlink info directly, we'll convert to PathBuf for this check
            let pathbuf = PathBuf::from(vfs_path.as_str());
            if pathbuf.is_symlink() {
                if let Ok(target) = std::fs::read_link(&pathbuf) {
                    // Convert the symlink target back to VFS path
                    let fs = vfs::PhysicalFS::new("/");
                    let root = VfsPath::new(fs);
                    root.join(target.to_string_lossy().as_ref()).ok()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}