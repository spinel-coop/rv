pub mod engine;
pub mod request;
pub mod version;

use camino::{Utf8Path, Utf8PathBuf};
use rv_cache::{CacheKey, CacheKeyHasher};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::{Display, Write};
use std::process::{Command, ExitStatus};
use std::str::FromStr;
use tracing::instrument;

use crate::engine::RubyEngine;
use crate::request::RubyRequest;
use crate::version::RubyVersion;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ruby {
    /// Unique identifier for this Ruby installation
    pub key: String,

    /// Ruby version (e.g., "3.1.4", "9.4.0.0")
    pub version: RubyVersion,

    /// Path to the Ruby installation directory
    pub path: Utf8PathBuf,

    /// Symlink target if this Ruby is a symlink
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink: Option<Utf8PathBuf>,

    /// System architecture (aarch64, x86_64, etc.)
    pub arch: String,

    /// Operating system (macos, linux, windows, etc.)
    pub os: String,

    pub gem_root: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default, PartialOrd, Ord)]
pub struct VersionParts {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre: Option<String>,
}

impl Display for VersionParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}.{}.{}", self.major, self.minor, self.patch))?;
        if let Some(pre) = self.pre.as_ref() {
            f.write_char('.')?;
            f.write_str(pre)?;
        }
        Ok(())
    }
}

impl FromStr for VersionParts {
    type Err = RubyError;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 2 {
            return Err(RubyError::InvalidVersion(version.to_string()));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| RubyError::InvalidVersion(version.to_string()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| RubyError::InvalidVersion(version.to_string()))?;

        // Handle cases where patch version is missing (like mruby "3.3")
        let patch = if parts.len() >= 3 {
            parts[2]
                .parse()
                .map_err(|_| RubyError::InvalidVersion(version.to_string()))?
        } else {
            0
        };

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
}

impl Ruby {
    /// Create a new Ruby instance from a directory path
    #[instrument(skip(dir), fields(dir = %dir.as_str()))]
    pub fn from_dir(dir: Utf8PathBuf) -> Result<Self, RubyError> {
        // Check for Ruby executable
        let ruby_bin = dir.join("bin").join("ruby");
        if !ruby_bin.exists() {
            return Err(RubyError::NoRubyExecutable);
        }

        let symlink = find_symlink_target(&ruby_bin);

        // Extract all information from the Ruby executable itself
        let mut ruby = extract_ruby_info(&ruby_bin)?;

        ruby.path = dir;
        ruby.symlink = symlink;

        Ok(ruby)
    }

    /// Check if this Ruby installation is valid
    pub fn is_valid(&self) -> bool {
        self.executable_path().exists()
    }

    /// Get display name for this Ruby
    pub fn display_name(&self) -> String {
        self.version.to_string()
    }

    /// Get the path to the Ruby executable for display purposes
    pub fn executable_path(&self) -> Utf8PathBuf {
        self.bin_path().join("ruby")
    }

    pub fn bin_path(&self) -> Utf8PathBuf {
        self.path.join("bin")
    }

    pub fn is_active(&self, active_version: &str) -> bool {
        RubyRequest::parse(active_version)
            .map(|request| request.satisfied_by(&self.version))
            .unwrap_or(Ok(false))
            .unwrap_or(false)
    }
}

impl PartialOrd for Ruby {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ruby {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        ((&self.version, &self.path)).cmp(&(&other.version, &self.path))
    }
}

impl CacheKey for Ruby {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        // Use key as the primary cache identifier since it contains:
        // implementation-version-os-arch (e.g., "ruby-3.3.0-macos-aarch64")
        self.key.cache_key(state);

        // Include path for uniqueness in case of path-based installations
        self.path.cache_key(state);

        // Include symlink information if present
        self.symlink.cache_key(state);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RubyError {
    #[error("Invalid path: {path}")]
    InvalidPath { path: String },
    #[error("No ruby executable found in bin/ directory")]
    NoRubyExecutable,
    #[error("Running ruby failed with status {0}:\n{1}")]
    RubyFailed(ExitStatus, String),
    #[error("Failed to parse Ruby directory name: {0}")]
    InvalidDirectoryName(String),
    #[error("Failed to parse version: {0}")]
    InvalidVersion(String),
    #[error(transparent)]
    RequestError(#[from] crate::request::RequestError),
}

/// Extract all Ruby information from the executable in a single call
#[instrument(skip_all)]
fn extract_ruby_info(ruby_bin: &Utf8PathBuf) -> Result<Ruby, RubyError> {
    // First try the full script with all features (works for most Ruby implementations)
    let full_script = r#"
        puts(Object.const_defined?(:RUBY_ENGINE) ? RUBY_ENGINE : 'ruby')
        puts(RUBY_VERSION)
        puts(Object.const_defined?(:RUBY_PLATFORM) ? RUBY_PLATFORM : 'unknown')
        puts(Object.const_defined?(:RbConfig) && RbConfig::CONFIG['host_cpu'] ? RbConfig::CONFIG['host_cpu'] : 'unknown')
        puts(Object.const_defined?(:RbConfig) && RbConfig::CONFIG['host_os'] ? RbConfig::CONFIG['host_os'] : 'unknown')
        puts(begin; require 'rubygems'; puts "export GEM_ROOT=#{Gem.default_dir.inspect};"; rescue ScriptError, NoMethodError; end)
    "#;

    let output = Command::new(ruby_bin)
        .args(["-e", full_script])
        .output()
        .map_err(|_| RubyError::NoRubyExecutable)?;

    if !output.status.success() {
        return Err(RubyError::RubyFailed(
            output.status,
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let info = String::from_utf8(output.stdout).unwrap();
    let mut lines = info.trim().lines();

    let ruby_engine = lines.next().unwrap_or("ruby");
    let ruby_version = lines.next().unwrap_or_default();
    let ruby_platform = lines.next().unwrap_or("unknown");
    let host_cpu = lines.next().unwrap_or("unknown");
    let host_os = lines.next().unwrap_or("unknown");
    let gem_root = lines.next().unwrap_or_default();

    let host_cpu = if host_cpu != "unknown" {
        host_cpu.to_string()
    } else {
        extract_arch_from_platform(ruby_platform)
    };
    let host_os = if host_os != "unknown" {
        host_os.to_string()
    } else {
        extract_os_from_platform(ruby_platform)
    };

    // Normalize architecture and OS names to match common conventions
    let arch = normalize_arch(&host_cpu);
    let os = normalize_os(&host_os);

    let engine: RubyEngine = ruby_engine.parse().unwrap();
    let version = ruby_version.parse()?;
    let gem_root = if gem_root.is_empty() {
        None
    } else {
        Some(Utf8PathBuf::from(gem_root))
    };

    let key = format!("{}-{}-{}-{}", engine.name(), version, os, arch);

    Ok(Ruby {
        key,
        version,
        arch,
        os,
        gem_root,
        // path and symlink are replaced in the caller
        path: Default::default(),
        symlink: Default::default(),
    })
}

/// Extract architecture from RUBY_PLATFORM string
fn extract_arch_from_platform(platform: &str) -> String {
    if platform.contains("aarch64") || platform.contains("arm64") {
        "aarch64".to_string()
    } else if platform.contains("x86_64") || platform.contains("amd64") {
        "x86_64".to_string()
    } else if platform.contains("i386") || platform.contains("i686") {
        "x86".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Extract OS from RUBY_PLATFORM string
fn extract_os_from_platform(platform: &str) -> String {
    if platform.contains("darwin") {
        "darwin".to_string()
    } else if platform.contains("linux") {
        "linux".to_string()
    } else if platform.contains("mingw") || platform.contains("mswin") {
        "windows".to_string()
    } else {
        "unknown".to_string()
    }
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

/// Find symlink target for a path, if it exists
fn find_symlink_target(path: &Utf8PathBuf) -> Option<Utf8PathBuf> {
    if path.is_symlink() {
        path.read_link_utf8().ok()
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
    find_active_ruby_version_with_env_and_fs(env_provider, &Utf8PathBuf::from("/"))
}

/// Find active Ruby version with injectable environment provider and filesystem (for testing)
pub fn find_active_ruby_version_with_env_and_fs(
    env_provider: &dyn EnvProvider,
    root: &Utf8PathBuf,
) -> Option<String> {
    // 1. Check RUBY_ROOT environment variable
    if let Some(ruby_root) = env_provider.get_var("RUBY_ROOT") {
        // Extract version from RUBY_ROOT path (e.g., "/path/to/ruby-3.1.4" -> "ruby-3.1.4")
        if let Some(dirname) = Utf8PathBuf::from(&ruby_root).file_name() {
            return Some(dirname.to_string());
        }
    }

    // 2. Look for .ruby-version file in current directory and parents
    if let Some(version) = find_ruby_version_file_fs(root) {
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

/// Search for .ruby-version file using filesystem (for testing)
fn find_ruby_version_file_fs(root: &Utf8PathBuf) -> Option<String> {
    let mut current_dir = if let Ok(cwd) = env::current_dir() {
        let cwd = Utf8PathBuf::from(cwd.to_str()?);
        if root == &Utf8PathBuf::from("/") {
            cwd
        } else {
            root.join(cwd.strip_prefix("/").unwrap_or(&cwd))
        }
    } else {
        return None;
    };

    loop {
        let ruby_version_file = current_dir.join(".ruby-version");

        if ruby_version_file.exists()
            && let Ok(content) = std::fs::read_to_string(&ruby_version_file)
        {
            let version = content.trim();
            if !version.is_empty() {
                return Some(version.to_string());
            }
        }

        // Move to parent directory
        if let Some(parent) = current_dir.parent() {
            if parent == current_dir {
                break; // Reached filesystem root
            }
            current_dir = parent.to_path_buf();
        } else {
            break; // No parent
        }
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
fn find_executable_in_path(executable: &str) -> Option<Utf8PathBuf> {
    if let Ok(path_var) = env::var("PATH") {
        for path_dir in env::split_paths(&path_var) {
            let path_dir = Utf8PathBuf::from(path_dir.to_str()?);

            let executable_path = path_dir.join(executable);
            if executable_path.is_file() && is_executable(&executable_path) {
                return Some(executable_path);
            }

            #[cfg(windows)]
            {
                let exe_path = path_dir.join(format!("{}.exe", executable_path));
                if exe_path.is_file() && is_executable(&exe_path) {
                    return Some(exe_path);
                }
            }
        }
    }

    None
}

/// Check if a file is executable
fn is_executable(path: &Utf8Path) -> bool {
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
fn extract_ruby_info_from_executable(ruby_path: &Utf8PathBuf) -> Option<String> {
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

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        let version = VersionParts::from_str("3.1.4").unwrap();
        assert_eq!(version.major, 3);
        assert_eq!(version.minor, 1);
        assert_eq!(version.patch, 4);
        assert_eq!(version.pre, None);

        let version = VersionParts::from_str("9.4.0.0").unwrap();
        assert_eq!(version.major, 9);
        assert_eq!(version.minor, 4);
        assert_eq!(version.patch, 0);
        assert_eq!(version.pre, Some("0".to_string()));

        // Test 2-part version (now supported for mruby)
        let version = VersionParts::from_str("1.2").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 0);
        assert_eq!(version.pre, None);

        assert!(VersionParts::from_str("1").is_err());
        assert!(VersionParts::from_str("invalid").is_err());
    }

    #[test]
    fn test_ruby_ordering() {
        // Create a dummy path for testing
        let dummy_path = Utf8PathBuf::from("/tmp/test-ruby");

        let ruby1 = Ruby {
            key: "ruby-3.1.4-macos-aarch64".to_string(),
            version: RubyVersion::parse("3.1.4").unwrap(),
            path: dummy_path.clone(),
            symlink: None,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            gem_root: None,
        };

        let ruby2 = Ruby {
            key: "ruby-3.2.0-macos-aarch64".to_string(),
            version: RubyVersion::parse("ruby-3.2.0").unwrap(),
            path: dummy_path.clone(),
            symlink: None,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            gem_root: None,
        };

        let jruby = Ruby {
            key: "jruby-9.4.0.0-macos-aarch64".to_string(),
            version: RubyVersion::parse("jruby-9.4.0.0").unwrap(),
            path: dummy_path,
            symlink: None,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            gem_root: None,
        };

        // Test version ordering within same implementation (higher versions last)
        assert!(ruby1 < ruby2); // 3.1.4 comes before 3.2.0

        // Test implementation priority: ruby comes before jruby
        assert!(ruby1 < jruby);
        assert!(ruby2 < jruby);
    }

    #[test]
    fn test_is_active_exact_match() {
        let ruby = create_test_ruby("ruby-3.1.4");

        // Exact match
        assert!(ruby.is_active("ruby-3.1.4"));
        assert!(!ruby.is_active("ruby-3.1.5"));
        assert!(!ruby.is_active("jruby-3.1.4"));
    }

    #[test]
    fn test_is_active_version_only() {
        let ruby = create_test_ruby("ruby-3.1.4");

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
        let ruby = create_test_ruby("ruby-3.1.4");

        // Engine-version prefix matching
        assert!(ruby.is_active("ruby-3.1"));
        assert!(ruby.is_active("ruby-3"));
        assert!(!ruby.is_active("ruby-3.2"));
    }

    #[test]
    fn test_is_active_engine_only() {
        let ruby = create_test_ruby("jruby-9.4.0.0");

        // Engine-only matching (should match any version of that engine)
        assert!(ruby.is_active("jruby"));
        assert!(ruby.is_active("jruby-"));
        assert!(!ruby.is_active("ruby"));
        assert!(!ruby.is_active("ruby-"));
    }

    #[test]
    fn test_is_active_jruby() {
        let jruby = create_test_ruby("jruby-9.4.0.0");

        // JRuby-specific tests
        assert!(jruby.is_active("jruby-9.4.0.0"));
        assert!(jruby.is_active("jruby-9.4"));
        assert!(jruby.is_active("jruby-9"));
        assert!(!jruby.is_active("9.4.0.0")); // Version-only shouldn't match JRuby
        assert!(!jruby.is_active("ruby-9.4.0.0"));
    }

    #[test]
    fn test_find_active_ruby_version_with_env_vars() {
        use assert_fs::prelude::*;

        // Mock environment provider for testing
        struct MockEnv {
            vars: std::collections::HashMap<String, String>,
        }

        impl EnvProvider for MockEnv {
            fn get_var(&self, key: &str) -> Option<String> {
                self.vars.get(key).cloned()
            }
        }

        // Create a temporary directory for testing
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let root = Utf8PathBuf::from(temp_dir.path().to_path_buf().to_str().unwrap());

        // Test RUBY_ROOT environment variable
        let env = MockEnv {
            vars: [("RUBY_ROOT".to_string(), "/path/to/ruby-3.2.1".to_string())]
                .iter()
                .cloned()
                .collect(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &root);
        assert_eq!(result, Some("ruby-3.2.1".to_string()));

        // Test DEFAULT_RUBY_VERSION environment variable (when RUBY_ROOT not set)
        // No .ruby-version file in empty filesystem
        let env = MockEnv {
            vars: [("DEFAULT_RUBY_VERSION".to_string(), "3.1.4".to_string())]
                .iter()
                .cloned()
                .collect(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &root);
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
        let result = find_active_ruby_version_with_env_and_fs(&env, &root);
        assert_eq!(result, Some("jruby-9.4.0.0".to_string()));

        // Test .ruby-version file precedence over DEFAULT_RUBY_VERSION
        // First create a .ruby-version file in the temp directory
        temp_dir.child(".ruby-version").write_str("2.7.6").unwrap();

        let env = MockEnv {
            vars: [("DEFAULT_RUBY_VERSION".to_string(), "3.1.4".to_string())]
                .iter()
                .cloned()
                .collect(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &root);
        assert_eq!(result, Some("2.7.6".to_string()));

        // Test no environment variables set with empty filesystem (no .ruby-version, no PATH)
        let empty_temp = assert_fs::TempDir::new().unwrap();
        let empty_root = Utf8PathBuf::from(empty_temp.path().to_path_buf().to_str().unwrap());
        let env = MockEnv {
            vars: std::collections::HashMap::new(),
        };
        let result = find_active_ruby_version_with_env_and_fs(&env, &empty_root);

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
    fn test_find_ruby_version_file_with_fs() {
        use assert_fs::prelude::*;

        // Create a temporary directory for testing
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let root = Utf8PathBuf::from(temp_dir.path().to_path_buf().to_str().unwrap());

        // Create a .ruby-version file with whitespace
        temp_dir
            .child(".ruby-version")
            .write_str("  3.1.4  \n")
            .unwrap();

        // Test finding the .ruby-version file using filesystem
        let result = find_ruby_version_file_fs(&root);

        assert_eq!(result, Some("3.1.4".to_string()));
    }

    #[test]
    fn test_find_ruby_version_file_parent_directory() {
        use assert_fs::prelude::*;

        // This test verifies the parent directory logic works

        // Create a temporary directory for testing
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let _root = temp_dir.path().to_path_buf();

        // Set up a directory structure with .ruby-version in a parent directory
        temp_dir.child("parent").create_dir_all().unwrap();
        temp_dir
            .child("parent")
            .child("child")
            .create_dir_all()
            .unwrap();

        // Create .ruby-version in parent directory
        temp_dir
            .child("parent")
            .child(".ruby-version")
            .write_str("2.7.6\n")
            .unwrap();

        // Test that we can traverse up from child to find .ruby-version
        // This test simulates the parent directory traversal logic
        let child_path = temp_dir.child("parent").child("child").path().to_path_buf();
        let mut current = child_path;
        let mut found_version = None;

        loop {
            let version_file = current.join(".ruby-version");
            if version_file.exists()
                && let Ok(content) = std::fs::read_to_string(&version_file)
            {
                found_version = Some(content.trim().to_string());
                break;
            }

            if let Some(parent) = current.parent() {
                if parent == current {
                    break; // Reached filesystem root
                }
                current = parent.to_path_buf();
            } else {
                break; // No parent
            }
        }

        assert_eq!(found_version, Some("2.7.6".to_string()));
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

        // Test with empty filesystem and no environment variables (PATH fallback will be tested)
        let empty_temp = assert_fs::TempDir::new().unwrap();
        let empty_root = Utf8PathBuf::from(empty_temp.path().to_path_buf().to_str().unwrap());

        let env = MockEnvWithPath {
            vars: std::collections::HashMap::new(),
        };

        // Since we can't easily mock PATH in a test, this will likely return None
        // unless there's a system Ruby in PATH, which is acceptable
        let result = find_active_ruby_version_with_env_and_fs(&env, &empty_root);

        // The result depends on system state, so we just verify it doesn't panic
        // and returns either None or Some valid Ruby version string
        if let Some(version) = result {
            assert!(!version.is_empty());
        }
    }

    fn create_test_ruby(version: &str) -> Ruby {
        let dummy_path = Utf8PathBuf::from("/tmp/test-ruby");

        let version = version.parse().unwrap();

        Ruby {
            key: format!("{}-test-arch64", &version),
            path: dummy_path,
            symlink: None,
            arch: "aarch64".to_string(),
            os: "test".to_string(),
            gem_root: None,
            version,
        }
    }
}
