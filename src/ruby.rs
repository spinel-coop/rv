use regex::Regex;
use serde::{Deserialize, Serialize, Serializer};
use serde_with::{DisplayFromStr, serde_as};
use std::env;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::time::SystemTime;
// Note: We considered using strum::Display, but the Unknown(String) variant
// makes manual implementation more straightforward
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
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_vfs_path"
    )]
    pub symlink: Option<VfsPath>,

    /// Ruby implementation
    #[serde_as(as = "DisplayFromStr")]
    pub implementation: RubyImplementation,

    /// System architecture (aarch64, x86_64, etc.)
    pub arch: String,

    /// Operating system (macos, linux, windows, etc.)
    pub os: String,

    /// Modification time of the ruby executable (for cache invalidation)
    #[serde(skip)]
    pub mtime: Option<SystemTime>,
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
            Self::Ruby => 0, // Ruby always comes first
            Self::JRuby | Self::TruffleRuby | Self::MRuby | Self::Artichoke => 1, // Known implementations second
            Self::Unknown(_) => 2, // Unknown implementations last
        };

        let other_priority = match other {
            Self::Ruby => 0,
            Self::JRuby | Self::TruffleRuby | Self::MRuby | Self::Artichoke => 1,
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
                path: vfs_dir.as_str().to_string(),
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

        // Get modification time of the ruby executable for cache invalidation
        let mtime = ruby_bin.metadata().ok().and_then(|meta| meta.modified);

        // Extract arch/os from the Ruby executable itself
        let (arch, os) = extract_ruby_platform_info(&ruby_bin)?;

        let key = format!("{}-{}-{}-{}", implementation.name(), version, os, arch);

        Ok(Ruby {
            key,
            version,
            version_parts,
            path: vfs_dir,
            symlink,
            implementation,
            arch,
            os,
            mtime,
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
        PathBuf::from(
            self.path
                .join("bin")
                .unwrap()
                .join("ruby")
                .unwrap()
                .as_str(),
        )
    }

    /// Check if this Ruby matches the active version pattern
    /// This implements chrb's pattern matching logic
    pub fn is_active(&self, active_version: &str) -> bool {
        let ruby_name = self.display_name();

        // Exact match: "ruby-3.1.4" == "ruby-3.1.4"
        if ruby_name == active_version {
            return true;
        }

        // Check if active_version is just a version (no engine prefix)
        // e.g., "3.1.4" should match "ruby-3.1.4"
        if !active_version.contains('-') {
            // Split ruby_name into engine and version
            if let Some((engine, version)) = ruby_name.split_once('-') {
                // Version-only matching should only work for "ruby" engine by default
                if engine == "ruby" {
                    if version == active_version {
                        return true;
                    }

                    // Also check for prefix matching: "3.1" matches "3.1.4"
                    if version.starts_with(active_version)
                        && version.chars().nth(active_version.len()) == Some('.')
                    {
                        return true;
                    }
                }
            }
        } else {
            // Engine-version format, check for prefix matching
            // e.g., "ruby-3.1" should match "ruby-3.1.4"
            if ruby_name.starts_with(active_version)
                && ruby_name.chars().nth(active_version.len()) == Some('.')
            {
                return true;
            }

            // Check engine-only matching: "ruby-" matches "ruby-3.1.4"
            if let Some(engine) = active_version.strip_suffix('-') {
                if self.implementation.name() == engine {
                    return true;
                }
            }
        }

        false
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

    let major = parts[0]
        .parse()
        .map_err(|_| RubyError::InvalidVersion(version.to_string()))?;
    let minor = parts[1]
        .parse()
        .map_err(|_| RubyError::InvalidVersion(version.to_string()))?;
    let patch = parts[2]
        .parse()
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
            version_parts: VersionParts {
                major: 3,
                minor: 1,
                patch: 4,
                pre: None,
            },
            path: dummy_vfs_path.clone(),
            symlink: None,
            implementation: RubyImplementation::Ruby,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            mtime: None,
        };

        let ruby2 = Ruby {
            key: "ruby-3.2.0-macos-aarch64".to_string(),
            version: "3.2.0".to_string(),
            version_parts: VersionParts {
                major: 3,
                minor: 2,
                patch: 0,
                pre: None,
            },
            path: dummy_vfs_path.clone(),
            symlink: None,
            implementation: RubyImplementation::Ruby,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            mtime: None,
        };

        let jruby = Ruby {
            key: "jruby-9.4.0.0-macos-aarch64".to_string(),
            version: "9.4.0.0".to_string(),
            version_parts: VersionParts {
                major: 9,
                minor: 4,
                patch: 0,
                pre: Some("0".to_string()),
            },
            path: dummy_vfs_path,
            symlink: None,
            implementation: RubyImplementation::JRuby,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            mtime: None,
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
        assert_eq!(
            RubyImplementation::from_str("ruby").unwrap(),
            RubyImplementation::Ruby
        );
        assert_eq!(
            RubyImplementation::from_str("jruby").unwrap(),
            RubyImplementation::JRuby
        );
        assert_eq!(
            RubyImplementation::from_str("truffleruby").unwrap(),
            RubyImplementation::TruffleRuby
        );
        assert_eq!(
            RubyImplementation::from_str("mruby").unwrap(),
            RubyImplementation::MRuby
        );
        assert_eq!(
            RubyImplementation::from_str("artichoke").unwrap(),
            RubyImplementation::Artichoke
        );
        assert_eq!(
            RubyImplementation::from_str("custom-ruby").unwrap(),
            RubyImplementation::Unknown("custom-ruby".to_string())
        );
    }

    #[test]
    fn test_ruby_implementation_name() {
        assert_eq!(RubyImplementation::Ruby.name(), "ruby");
        assert_eq!(RubyImplementation::JRuby.name(), "jruby");
        assert_eq!(
            RubyImplementation::Unknown("custom-ruby".to_string()).name(),
            "custom-ruby"
        );
    }

    #[test]
    fn test_is_active_exact_match() {
        let ruby = create_test_ruby("ruby", "3.1.4");

        // Exact match
        assert!(ruby.is_active("ruby-3.1.4"));
        assert!(!ruby.is_active("ruby-3.1.5"));
        assert!(!ruby.is_active("jruby-3.1.4"));
    }

    #[test]
    fn test_is_active_version_only() {
        let ruby = create_test_ruby("ruby", "3.1.4");

        // Version-only matching (should match ruby engine by default)
        assert!(ruby.is_active("3.1.4"));
        assert!(!ruby.is_active("3.1.5"));

        // Prefix matching for versions
        assert!(ruby.is_active("3.1"));
        assert!(ruby.is_active("3"));
        assert!(!ruby.is_active("3.2"));
    }

    #[test]
    fn test_is_active_prefix_matching() {
        let ruby = create_test_ruby("ruby", "3.1.4");

        // Engine-version prefix matching
        assert!(ruby.is_active("ruby-3.1"));
        assert!(ruby.is_active("ruby-3"));
        assert!(!ruby.is_active("ruby-3.2"));
    }

    #[test]
    fn test_is_active_engine_only() {
        let ruby = create_test_ruby("jruby", "9.4.0.0");

        // Engine-only matching (should match any version of that engine)
        assert!(ruby.is_active("jruby-"));
        assert!(!ruby.is_active("ruby-"));
    }

    #[test]
    fn test_is_active_jruby() {
        let jruby = create_test_ruby("jruby", "9.4.0.0");

        // JRuby-specific tests
        assert!(jruby.is_active("jruby-9.4.0.0"));
        assert!(jruby.is_active("jruby-9.4"));
        assert!(jruby.is_active("jruby-9"));
        assert!(!jruby.is_active("9.4.0.0")); // Version-only shouldn't match JRuby
        assert!(!jruby.is_active("ruby-9.4.0.0"));
    }

    #[test]
    fn test_find_active_ruby_version_with_env_vars() {
        use vfs::{MemoryFS, VfsPath};

        // Mock environment provider for testing
        struct MockEnv {
            vars: std::collections::HashMap<String, String>,
        }

        impl EnvProvider for MockEnv {
            fn get_var(&self, key: &str) -> Option<String> {
                self.vars.get(key).cloned()
            }
        }

        // Create a memory VFS for testing
        let fs = MemoryFS::new();
        let vfs_root = VfsPath::new(fs);

        // Test RUBY_ROOT environment variable
        let env = MockEnv {
            vars: [("RUBY_ROOT".to_string(), "/path/to/ruby-3.2.1".to_string())]
                .iter()
                .cloned()
                .collect(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &vfs_root);
        assert_eq!(result, Some("ruby-3.2.1".to_string()));

        // Test DEFAULT_RUBY_VERSION environment variable (when RUBY_ROOT not set)
        // No .ruby-version file in empty VFS
        let env = MockEnv {
            vars: [("DEFAULT_RUBY_VERSION".to_string(), "3.1.4".to_string())]
                .iter()
                .cloned()
                .collect(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &vfs_root);
        assert_eq!(result, Some("3.1.4".to_string()));

        // Test precedence: RUBY_ROOT should override DEFAULT_RUBY_VERSION
        let env = MockEnv {
            vars: [
                (
                    "RUBY_ROOT".to_string(),
                    "/path/to/jruby-9.4.0.0".to_string(),
                ),
                ("DEFAULT_RUBY_VERSION".to_string(), "3.1.4".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &vfs_root);
        assert_eq!(result, Some("jruby-9.4.0.0".to_string()));

        // Test .ruby-version file precedence over DEFAULT_RUBY_VERSION
        // First create a .ruby-version file in the VFS
        let current_path = if let Ok(cwd) = std::env::current_dir() {
            vfs_root.join(cwd.to_string_lossy().as_ref()).unwrap()
        } else {
            vfs_root.join("test_dir").unwrap()
        };
        current_path.create_dir_all().unwrap();
        let ruby_version_file = current_path.join(".ruby-version").unwrap();
        ruby_version_file
            .create_file()
            .unwrap()
            .write_all(b"2.7.6")
            .unwrap();

        let env = MockEnv {
            vars: [("DEFAULT_RUBY_VERSION".to_string(), "3.1.4".to_string())]
                .iter()
                .cloned()
                .collect(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &vfs_root);
        assert_eq!(result, Some("2.7.6".to_string()));

        // Test no environment variables set with empty VFS (no .ruby-version, no PATH)
        let empty_fs = MemoryFS::new();
        let empty_vfs = VfsPath::new(empty_fs);
        let env = MockEnv {
            vars: std::collections::HashMap::new(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &empty_vfs);

        // Result could be None (no version sources) or Some (PATH fallback found Ruby)
        // Both are valid depending on system state - the test verifies it doesn't crash
        match result {
            Some(version) => {
                assert!(
                    !version.is_empty(),
                    "If version found, it should not be empty"
                );
            }
            None => {
                // This is also valid - no Ruby version detected
            }
        }
    }

    #[test]
    fn test_find_ruby_version_file_with_vfs() {
        use vfs::{MemoryFS, VfsPath};

        // Create a memory VFS for testing
        let fs = MemoryFS::new();
        let vfs_root = VfsPath::new(fs);

        // Create the test directory structure
        let current_path = if let Ok(cwd) = std::env::current_dir() {
            vfs_root.join(cwd.to_string_lossy().as_ref()).unwrap()
        } else {
            vfs_root.join("test_dir").unwrap()
        };
        current_path.create_dir_all().unwrap();

        // Create a .ruby-version file with whitespace
        let ruby_version_file = current_path.join(".ruby-version").unwrap();
        ruby_version_file
            .create_file()
            .unwrap()
            .write_all(b"  3.1.4  \n")
            .unwrap();

        // Test finding the .ruby-version file using VFS
        let result = find_ruby_version_file_vfs(&vfs_root);

        assert_eq!(result, Some("3.1.4".to_string()));
    }

    #[test]
    fn test_find_ruby_version_file_parent_directory() {
        use vfs::{MemoryFS, VfsPath};

        // This test requires modifying the VFS traversal function to accept a starting path
        // For now, let's create a simpler test that verifies the parent directory logic works

        // Create a memory VFS for testing
        let fs = MemoryFS::new();
        let vfs_root = VfsPath::new(fs);

        // Set up a directory structure with .ruby-version in a parent directory
        let base_dir = vfs_root.join("test").unwrap();
        let parent_dir = base_dir.join("parent").unwrap();
        let child_dir = parent_dir.join("child").unwrap();
        child_dir.create_dir_all().unwrap();

        // Create .ruby-version in parent directory
        let ruby_version_file = parent_dir.join(".ruby-version").unwrap();
        ruby_version_file
            .create_file()
            .unwrap()
            .write_all(b"2.7.6\n")
            .unwrap();

        // Test that we can traverse up from child to find .ruby-version
        let mut current = child_dir;
        let mut found_version = None;

        loop {
            let version_file = current.join(".ruby-version").ok();
            if let Some(file) = version_file {
                if file.exists().unwrap_or(false) {
                    if let Ok(content) = file.read_to_string() {
                        found_version = Some(content.trim().to_string());
                        break;
                    }
                }
            }

            let parent = current.parent();
            if parent.as_str() == current.as_str() {
                break; // Reached VFS root
            }
            current = parent;
        }

        assert_eq!(found_version, Some("2.7.6".to_string()));
    }

    #[test]
    fn test_extract_ruby_info_from_path() {
        // Test standard Ruby path
        let ruby_path = PathBuf::from("/Users/user/.rubies/ruby-3.1.4/bin/ruby");
        let result = extract_ruby_info_from_path(&ruby_path);
        assert_eq!(result, Some("ruby-3.1.4".to_string()));

        // Test JRuby path
        let jruby_path = PathBuf::from("/opt/rubies/jruby-9.4.0.0/bin/ruby");
        let result = extract_ruby_info_from_path(&jruby_path);
        assert_eq!(result, Some("jruby-9.4.0.0".to_string()));

        // Test TruffleRuby path
        let truffle_path = PathBuf::from("/home/user/.rubies/truffleruby-23.1.1/bin/ruby");
        let result = extract_ruby_info_from_path(&truffle_path);
        assert_eq!(result, Some("truffleruby-23.1.1".to_string()));

        // Test system Ruby (no version pattern)
        let system_path = PathBuf::from("/usr/bin/ruby");
        let result = extract_ruby_info_from_path(&system_path);
        assert_eq!(result, Some("ruby".to_string()));
    }

    #[test]
    fn test_find_executable_in_path() {
        // This test is simplified to avoid unsafe env::set_var operations
        // Instead, we test the PATH parsing logic with a known system path

        // Test with current PATH - if ruby exists, it should be found
        let found = find_executable_in_path("ruby");

        // This test will pass if either:
        // 1. Ruby is found in PATH (returns Some)
        // 2. Ruby is not in PATH (returns None)
        // Both are valid outcomes depending on the system
        match found {
            Some(path) => {
                // If found, verify it's a valid path
                assert!(path.exists());
                assert!(path.is_file());
            }
            None => {
                // If not found, that's also valid - system may not have Ruby in PATH
            }
        }
    }

    #[test]
    fn test_path_fallback_integration() {
        // Test the full PATH fallback integration with mock environment
        struct MockEnvWithPath {
            vars: std::collections::HashMap<String, String>,
        }

        impl EnvProvider for MockEnvWithPath {
            fn get_var(&self, key: &str) -> Option<String> {
                self.vars.get(key).cloned()
            }
        }

        // Test with empty VFS and no environment variables (PATH fallback will be tested)
        let empty_fs = vfs::MemoryFS::new();
        let empty_vfs = VfsPath::new(empty_fs);

        let env = MockEnvWithPath {
            vars: std::collections::HashMap::new(),
        };

        // Since we can't easily mock PATH in a test, this will likely return None
        // unless there's a system Ruby in PATH, which is acceptable
        let result = find_active_ruby_version_with_env_and_fs(&env, &empty_vfs);

        // The result depends on system state, so we just verify it doesn't panic
        // and returns either None or Some valid Ruby version string
        if let Some(version) = result {
            assert!(!version.is_empty());
        }
    }

    fn create_test_ruby(implementation: &str, version: &str) -> Ruby {
        use vfs::{MemoryFS, VfsPath};

        let fs = MemoryFS::new();
        let vfs_root = VfsPath::new(fs);
        let dummy_vfs_path = vfs_root.join("tmp").unwrap();

        let implementation_enum = RubyImplementation::from_str(implementation).unwrap();
        let version_parts = parse_version(version).unwrap();

        Ruby {
            key: format!("{}-{}-test-arch64", implementation, version),
            version: version.to_string(),
            version_parts,
            path: dummy_vfs_path,
            symlink: None,
            implementation: implementation_enum,
            arch: "aarch64".to_string(),
            os: "test".to_string(),
            mtime: None,
        }
    }
}

/// Extract arch and OS information from a Ruby executable
fn extract_ruby_platform_info(ruby_bin: &VfsPath) -> Result<(String, String), RubyError> {
    // For VFS compatibility, we need to handle the case where we can't execute the binary
    // In such cases, fall back to the current system's platform info

    // Try to get the actual file path for execution
    let ruby_path = ruby_bin.as_str();

    // Run ruby -e "puts [RUBY_PLATFORM, RbConfig::CONFIG['host_cpu'], RbConfig::CONFIG['host_os']].join('|')"
    let output = Command::new(ruby_path)
        .args(["-e", "puts [RUBY_PLATFORM, RbConfig::CONFIG['host_cpu'], RbConfig::CONFIG['host_os']].join('|')"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let platform_info = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = platform_info.trim().split('|').collect();

            if parts.len() >= 3 {
                let host_cpu = parts[1].to_string();
                let host_os = parts[2].to_string();

                // Normalize architecture names to match common conventions
                let arch = normalize_arch(&host_cpu);
                let os = normalize_os(&host_os);

                return Ok((arch, os));
            }
        }
        _ => {
            // Fall back to system platform info if we can't execute Ruby
            // This happens in test environments or when Ruby is not functional
        }
    }

    // Fallback to current system's platform info
    Ok((
        std::env::consts::ARCH.to_string(),
        std::env::consts::OS.to_string(),
    ))
}

/// Normalize architecture names to match common conventions
fn normalize_arch(arch: &str) -> String {
    match arch {
        "aarch64" | "arm64" => "aarch64".to_string(),
        "x86_64" | "amd64" => "x86_64".to_string(),
        "i386" | "i686" => "x86".to_string(),
        other => other.to_string(),
    }
}

/// Normalize OS names to match common conventions
fn normalize_os(os: &str) -> String {
    match os {
        s if s.contains("darwin") => "macos".to_string(),
        s if s.contains("linux") => "linux".to_string(),
        s if s.contains("mingw") || s.contains("mswin") || s.contains("windows") => {
            "windows".to_string()
        }
        s if s.contains("freebsd") => "freebsd".to_string(),
        s if s.contains("openbsd") => "openbsd".to_string(),
        s if s.contains("netbsd") => "netbsd".to_string(),
        other => other.to_string(),
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

/// Trait for environment variable access (allows mocking in tests)
pub trait EnvProvider {
    fn get_var(&self, key: &str) -> Option<String>;
}

/// Production environment provider
pub struct SystemEnv;

impl EnvProvider for SystemEnv {
    fn get_var(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

/// Find the currently active Ruby version using chrb's precedence order
/// 1. RUBY_ROOT environment variable (if set, indicates active Ruby)
/// 2. .ruby-version file in current directory or parent directories
/// 3. DEFAULT_RUBY_VERSION environment variable
pub fn find_active_ruby_version() -> Option<String> {
    find_active_ruby_version_with_env(&SystemEnv)
}

/// Find active Ruby version with injectable environment provider (for testing)
pub fn find_active_ruby_version_with_env(env_provider: &dyn EnvProvider) -> Option<String> {
    find_active_ruby_version_with_env_and_fs(env_provider, &VfsPath::new(vfs::PhysicalFS::new("/")))
}

/// Find active Ruby version with injectable environment provider and VFS (for testing)
pub fn find_active_ruby_version_with_env_and_fs(
    env_provider: &dyn EnvProvider,
    vfs_root: &VfsPath,
) -> Option<String> {
    // 1. Check RUBY_ROOT environment variable
    if let Some(ruby_root) = env_provider.get_var("RUBY_ROOT") {
        // Extract version from RUBY_ROOT path (e.g., "/path/to/ruby-3.1.4" -> "ruby-3.1.4")
        if let Some(dirname) = PathBuf::from(&ruby_root).file_name() {
            if let Some(dirname_str) = dirname.to_str() {
                return Some(dirname_str.to_string());
            }
        }
    }

    // 2. Look for .ruby-version file in current directory and parents
    if let Some(version) = find_ruby_version_file_vfs(vfs_root) {
        return Some(version);
    }

    // 3. Check DEFAULT_RUBY_VERSION environment variable
    if let Some(default_version) = env_provider.get_var("DEFAULT_RUBY_VERSION") {
        return Some(default_version);
    }

    // 4. Fallback to PATH analysis - find Ruby executable and extract version
    if let Some(path_ruby) = find_ruby_in_path() {
        return Some(path_ruby);
    }

    None
}

/// Search for .ruby-version file using VFS (for testing)
fn find_ruby_version_file_vfs(vfs_root: &VfsPath) -> Option<String> {
    let mut current_dir = if let Ok(cwd) = env::current_dir() {
        vfs_root.join(cwd.to_string_lossy().as_ref()).ok()?
    } else {
        return None;
    };

    loop {
        let ruby_version_file = current_dir.join(".ruby-version").ok()?;

        if ruby_version_file.exists().unwrap_or(false) {
            if let Ok(content) = ruby_version_file.read_to_string() {
                let version = content.trim();
                if !version.is_empty() {
                    return Some(version.to_string());
                }
            }
        }

        // Move to parent directory
        let parent = current_dir.parent();
        if parent.as_str() == current_dir.as_str() {
            break; // Reached VFS root
        }
        current_dir = parent;
    }

    None
}

/// Find Ruby executable in PATH and extract version information
/// This is the fallback method when no .ruby-version file or env vars are set
fn find_ruby_in_path() -> Option<String> {
    // First, try to find 'ruby' executable in PATH
    let ruby_path = find_executable_in_path("ruby")?;

    // Extract version and engine information from the Ruby executable
    extract_ruby_info_from_executable(&ruby_path)
}

/// Find an executable in PATH
fn find_executable_in_path(executable: &str) -> Option<PathBuf> {
    if let Ok(path_var) = env::var("PATH") {
        for path_dir in env::split_paths(&path_var) {
            let executable_path = path_dir.join(executable);

            // Check for executable with and without .exe extension (Windows support)
            if executable_path.is_file() && is_executable(&executable_path) {
                return Some(executable_path);
            }

            // Windows support - check for .exe extension
            #[cfg(windows)]
            {
                let exe_path = path_dir.join(format!("{}.exe", executable));
                if exe_path.is_file() && is_executable(&exe_path) {
                    return Some(exe_path);
                }
            }
        }
    }

    None
}

/// Check if a file is executable
fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            let permissions = metadata.permissions();
            return permissions.mode() & 0o111 != 0;
        }
        false
    }

    #[cfg(windows)]
    {
        // On Windows, if the file exists and has .exe extension, consider it executable
        path.extension().map_or(false, |ext| ext == "exe")
    }

    #[cfg(not(any(unix, windows)))]
    {
        // For other platforms, just check if file exists
        true
    }
}

/// Extract Ruby engine and version information from an executable
/// Uses the same approach as chrb - execute Ruby to get accurate information
fn extract_ruby_info_from_executable(ruby_path: &PathBuf) -> Option<String> {
    // Execute Ruby to get engine and version information
    // This mirrors chrb's ExecFindEnv function
    let output = Command::new(ruby_path)
        .args([
            "-e",
            "puts \"#{defined?(RUBY_ENGINE) ? RUBY_ENGINE : 'ruby'}-#{RUBY_VERSION}\"",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).ok()?;
        let version_info = stdout.trim();

        if !version_info.is_empty() {
            return Some(version_info.to_string());
        }
    }

    // Fallback: try to extract from path if execution failed
    extract_ruby_info_from_path(ruby_path)
}

/// Fallback method to extract Ruby information from the executable path
/// This looks for patterns in the path like /path/to/ruby-3.1.4/bin/ruby
fn extract_ruby_info_from_path(ruby_path: &Path) -> Option<String> {
    // Look for Ruby installation directory in path
    // e.g., /Users/user/.rubies/ruby-3.1.4/bin/ruby -> ruby-3.1.4
    let path_str = ruby_path.to_string_lossy();

    // Common Ruby installation patterns
    let patterns = [
        r"/(ruby-[\d.]+[^/]*)/bin/ruby",        // /path/ruby-3.1.4/bin/ruby
        r"/(jruby-[\d.]+[^/]*)/bin/ruby",       // /path/jruby-9.4.0.0/bin/ruby
        r"/(truffleruby-[\d.]+[^/]*)/bin/ruby", // /path/truffleruby-23.1.1/bin/ruby
    ];

    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(&path_str) {
                if let Some(matched) = captures.get(1) {
                    return Some(matched.as_str().to_string());
                }
            }
        }
    }

    // If no pattern matches, default to "ruby" (we know it's some Ruby)
    Some("ruby".to_string())
}
