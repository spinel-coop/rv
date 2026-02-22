pub mod engine;
pub mod request;
pub mod tool_consumable;
pub mod version;

use camino::{Utf8Path, Utf8PathBuf};
use once_cell::sync::Lazy;
use regex::Regex;
use rv_cache::{CacheKey, CacheKeyHasher};
use serde::{Deserialize, Serialize};
use std::env::{
    self,
    consts::{ARCH, OS},
};
use std::process::Command;
use tracing::instrument;

use crate::version::ReleasedRubyVersion;

/// Returns the possible Ruby executable names for the current platform, in priority order.
/// On Windows, checks `ruby.exe` (standard RubyInstaller2) then `ruby.cmd` (batch wrapper).
/// On Unix systems (macOS, Linux), it's just `ruby`.
fn ruby_executable_names() -> &'static [&'static str] {
    if cfg!(windows) {
        &["ruby.exe", "ruby.cmd"]
    } else {
        &["ruby"]
    }
}

/// Find the Ruby executable in a directory's `bin/` subdirectory.
/// Returns the first matching executable name from [`ruby_executable_names`].
pub fn find_ruby_executable(dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let bin_dir = dir.join("bin");
    for name in ruby_executable_names() {
        let path = bin_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

static RUBY_DESCRIPTION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"ruby (?<version>[^ ]+) \((?<date>\d\d\d\d-\d\d-\d\d)(?<time>T\d\d:\d\d:\d\dZ)? (?<source>\S+) (?<revision>[0-9a-f]+)\) (?<zjit>\+ZJIT )?(?<yjit>\+YJIT )?(?<prism>\+PRISM )?\[(?<arch>\w+)-(?<os>\w+)\]").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub name: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ruby {
    /// Unique identifier for this Ruby installation
    pub key: String,

    /// Ruby version (e.g., "3.1.4", "9.4.0.0")
    pub version: ReleasedRubyVersion,

    /// Path to the Ruby installation directory
    pub path: Utf8PathBuf,

    /// Whether this is a rv-managed version
    pub managed: bool,

    /// Symlink target if this Ruby is a symlink
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink: Option<Utf8PathBuf>,

    /// System architecture (aarch64, x86_64, etc.)
    pub arch: String,

    /// Operating system (macos, linux, windows, etc.)
    pub os: String,

    pub gem_root: Option<Utf8PathBuf>,
}

impl Ruby {
    /// Create a new Ruby instance from a directory path
    #[instrument(skip(dir, managed), fields(dir = %dir.as_str()), level = "trace")]
    pub fn from_dir(dir: Utf8PathBuf, managed: bool) -> Result<Self, RubyError> {
        let dir_name = dir.file_name().unwrap_or("");

        if dir_name.is_empty() {
            return Err(RubyError::InvalidPath {
                path: dir.to_string(),
            });
        }

        // Check for Ruby executable
        let ruby_bin = find_ruby_executable(&dir).ok_or(RubyError::NoRubyExecutable)?;

        let symlink = find_symlink_target(&ruby_bin);

        // Extract all information from the Ruby executable itself
        let mut ruby = extract_ruby_info(&ruby_bin)?;

        ruby.managed = managed;
        ruby.path = dir;
        ruby.symlink = symlink;

        Ok(ruby)
    }

    /// Check if this Ruby installation is valid
    pub fn is_valid(&self) -> bool {
        find_ruby_executable(&self.path).is_some()
    }

    /// Get the path to the Ruby executable.
    /// Checks for the first matching name from [`ruby_executable_names`].
    pub fn executable_path(&self) -> Utf8PathBuf {
        find_ruby_executable(&self.path)
            .unwrap_or_else(|| self.bin_path().join(ruby_executable_names()[0]))
    }

    pub fn bin_path(&self) -> Utf8PathBuf {
        self.path.join("bin")
    }

    pub fn gem_root(&self) -> Option<Utf8PathBuf> {
        self.gem_root.clone()
    }

    pub fn gem_home(&self) -> Utf8PathBuf {
        let home = rv_dirs::home_dir();
        let legacy_path = home.join(".gem").join(self.gem_scope());
        if legacy_path.exists() {
            legacy_path
        } else {
            home.join(".local")
                .join("share")
                .join("rv")
                .join("gems")
                .join(self.gem_scope())
        }
    }

    pub fn man_path(&self) -> Option<Utf8PathBuf> {
        let man_path = self.path.join("share/man");
        if man_path.is_dir() {
            Some(man_path)
        } else {
            None
        }
    }

    /// path scope for gems that can be safely shared with other rubies
    pub fn gem_scope(&self) -> String {
        format!("{}/{}", self.version.engine.name(), self.version.abi())
    }
}

impl PartialOrd for Ruby {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ruby {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&self.version, &self.managed, &self.path).cmp(&(
            &other.version,
            &other.managed,
            &other.path,
        ))
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
    #[error("Failed to run ruby executable in bin/ directory")]
    InvalidRubyExecutable,
    #[error("No ruby executable found in bin/ directory")]
    NoRubyExecutable,
    #[error("Failed to parse Ruby directory name: {0}")]
    InvalidDirectoryName(String),
    #[error("Failed to parse version: {0}")]
    InvalidVersion(String),
    #[error(transparent)]
    RequestError(#[from] crate::request::RequestError),
    #[error(transparent)]
    ParseVersionError(#[from] crate::version::ParseVersionError),
}

/// Extract all Ruby information from the executable in a single call
#[instrument(skip_all, level = "trace")]
fn extract_ruby_info(ruby_bin: &Utf8PathBuf) -> Result<Ruby, RubyError> {
    if ruby_bin.as_str().ends_with("0.49/bin/ruby") {
        return ruby_049_version();
    }

    // try the full script with all features (works for most Ruby implementations)
    let full_script = r#"
        puts(Object.const_defined?(:RUBY_ENGINE) ? RUBY_ENGINE : 'ruby')
        puts(Object.const_defined?(:RUBY_ENGINE_VERSION) ? RUBY_ENGINE_VERSION : RUBY_VERSION)
        puts(Object.const_defined?(:RUBY_PLATFORM) ? RUBY_PLATFORM : 'unknown')
        puts(Object.const_defined?(:RbConfig) && RbConfig::CONFIG['host_cpu'] ? RbConfig::CONFIG['host_cpu'] : 'unknown')
        puts(Object.const_defined?(:RbConfig) && RbConfig::CONFIG['host_os'] ? RbConfig::CONFIG['host_os'] : 'unknown')
        puts(begin; require 'rubygems'; Gem.default_dir; rescue ScriptError, NoMethodError; end)
        puts(Object.const_defined?(:RUBY_DESCRIPTION) ? RUBY_DESCRIPTION : '')
    "#;

    // On Windows, .cmd wrappers can't receive arguments containing special characters like (, ), ?
    // due to Rust's CVE-2024-24576 mitigation (1.77.2+). Following uv's pattern: write the probe
    // script to a temp file, then invoke through cmd.exe /c to bypass the restriction entirely.
    let output = if cfg!(windows) && ruby_bin.extension().is_some_and(|ext| ext == "cmd") {
        let probe_script = ruby_bin.with_file_name("_rv_probe.rb");
        std::fs::write(&probe_script, full_script).map_err(|_| RubyError::InvalidRubyExecutable)?;
        let result = Command::new("cmd")
            .args(["/c", ruby_bin.as_str(), probe_script.as_str()])
            .output();
        let _ = std::fs::remove_file(&probe_script);
        result.map_err(|_| RubyError::InvalidRubyExecutable)?
    } else {
        Command::new(ruby_bin)
            .args(["-e", full_script])
            .output()
            .map_err(|_| RubyError::InvalidRubyExecutable)?
    };

    let info = String::from_utf8(output.stdout).unwrap();
    let mut lines = info.trim().lines();

    let ruby_engine = lines.next().unwrap_or("ruby");
    let ruby_version = lines.next().unwrap_or_default();
    let ruby_platform = lines.next().unwrap_or("unknown");
    let host_cpu = lines.next().unwrap_or("unknown");
    let host_os = lines.next().unwrap_or("unknown");
    let gem_root = lines.next().unwrap_or_default();
    let description = lines.next().unwrap_or_default();
    let ruby_description = parse_description(description);

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

    let version: ReleasedRubyVersion = if let Some(d) = ruby_description {
        let desc_version = &d["version"];
        format!("{ruby_engine}-{desc_version}").parse()?
    } else {
        format!("{ruby_engine}-{ruby_version}").parse()?
    };
    let gem_root = if gem_root.is_empty() {
        None
    } else {
        Some(Utf8PathBuf::from(gem_root))
    };

    let key = format!("{version}-{os}-{arch}");

    Ok(Ruby {
        key,
        version,
        arch,
        os,
        gem_root,
        managed: false,
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

fn ruby_049_version() -> Result<Ruby, RubyError> {
    let version = "0.49.0".parse()?;
    let arch = normalize_arch(ARCH);
    let os = normalize_os(OS);
    let key = format!("{version}-{os}-{arch}");

    Ok(Ruby {
        key,
        version,
        arch,
        os,
        gem_root: None,
        managed: false,
        // path and symlink are replaced in the caller
        path: Default::default(),
        symlink: Default::default(),
    })
}

fn parse_description(description: &str) -> Option<regex::Captures<'_>> {
    RUBY_DESCRIPTION_REGEX.captures(description)
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

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_ruby_ordering() {
        // Create a dummy path for testing
        let dummy_path = Utf8PathBuf::from("/tmp/test-ruby");

        let ruby1 = Ruby {
            key: "ruby-3.1.4-macos-aarch64".to_string(),
            version: ReleasedRubyVersion::from_str("3.1.4").unwrap(),
            path: dummy_path.clone(),
            managed: false,
            symlink: None,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            gem_root: None,
        };

        let ruby2 = Ruby {
            key: "ruby-3.2.0-macos-aarch64".to_string(),
            version: ReleasedRubyVersion::from_str("ruby-3.2.0").unwrap(),
            path: dummy_path.clone(),
            managed: false,
            symlink: None,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            gem_root: None,
        };

        let ruby2_managed = Ruby {
            key: "ruby-3.2.0-macos-aarch64".to_string(),
            version: ReleasedRubyVersion::from_str("ruby-3.2.0").unwrap(),
            path: dummy_path.clone(),
            managed: true,
            symlink: None,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            gem_root: None,
        };

        let jruby = Ruby {
            key: "jruby-9.4.0.0-macos-aarch64".to_string(),
            version: ReleasedRubyVersion::from_str("jruby-9.4.0.0").unwrap(),
            path: dummy_path,
            managed: false,
            symlink: None,
            arch: "aarch64".to_string(),
            os: "macos".to_string(),
            gem_root: None,
        };

        // Test version ordering within same implementation (higher versions last)
        assert!(ruby1 < ruby2); // 3.1.4 comes before 3.2.0

        // Test version ordering within same version & implementation
        assert!(ruby2 < ruby2_managed); // Non managed versions come before managed versions

        // Test implementation priority: ruby comes before jruby
        assert!(ruby1 < jruby);
        assert!(ruby2 < jruby);
        assert!(ruby2_managed < jruby);
    }

    #[test]
    fn test_extract_ruby_info() {
        let ruby_path = Utf8PathBuf::from("/root/.local/share/rv/rubies/ruby-0.49/bin/ruby");
        let ruby = extract_ruby_info(&ruby_path).unwrap();
        assert_eq!(ruby.version.major, 0);
        assert_eq!(ruby.version.minor, 49);
        assert_eq!(ruby.version.patch, 0);
        assert_eq!(ruby.arch, ARCH);
    }

    #[test]
    fn test_parse_description() {
        let info =
            parse_description("ruby 3.1.6p260 (2024-05-29 revision a777087be6) [arm64-darwin24]")
                .unwrap();
        assert_eq!(&info["version"], "3.1.6p260");
        assert_eq!(&info["date"], "2024-05-29");
        assert_eq!(&info["source"], "revision");
        assert_eq!(&info["revision"], "a777087be6");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin24");

        let info =
            parse_description("ruby 3.2.9 (2025-07-24 revision 8f611e0c46) [arm64-darwin23]")
                .unwrap();
        assert_eq!(&info["version"], "3.2.9");
        assert_eq!(&info["date"], "2025-07-24");
        assert_eq!(&info["source"], "revision");
        assert_eq!(&info["revision"], "8f611e0c46");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin23");

        let info =
            parse_description("ruby 3.3.9 (2025-07-24 revision f5c772fc7c) [arm64-darwin23]")
                .unwrap();
        assert_eq!(&info["version"], "3.3.9");
        assert_eq!(&info["date"], "2025-07-24");
        assert_eq!(&info["source"], "revision");
        assert_eq!(&info["revision"], "f5c772fc7c");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin23");

        let info = parse_description(
            "ruby 3.4.0rc1 (2024-12-12 master 29caae9991) +PRISM [arm64-darwin25]",
        )
        .unwrap();
        assert_eq!(&info["version"], "3.4.0rc1");
        assert_eq!(&info["date"], "2024-12-12");
        assert_eq!(&info["source"], "master");
        assert_eq!(&info["revision"], "29caae9991");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin25");

        let info = parse_description(
            "ruby 3.4.7 (2025-10-08 revision 7a5688e2a2) +PRISM [arm64-darwin25]",
        )
        .unwrap();
        assert_eq!(&info["version"], "3.4.7");
        assert_eq!(&info["date"], "2025-10-08");
        assert_eq!(&info["source"], "revision");
        assert_eq!(&info["revision"], "7a5688e2a2");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin25");

        let info = parse_description(
            "ruby 3.5.0preview1 (2025-04-18 master d06ec25be4) +PRISM [arm64-darwin23]",
        )
        .unwrap();
        assert_eq!(&info["version"], "3.5.0preview1");
        assert_eq!(&info["date"], "2025-04-18");
        assert_eq!(&info["source"], "master");
        assert_eq!(&info["revision"], "d06ec25be4");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin23");

        let info = parse_description(
            "ruby 4.0.0preview2 (2025-11-17 master 4fa6e9938c) +PRISM [arm64-darwin23]",
        )
        .unwrap();
        assert_eq!(&info["version"], "4.0.0preview2");
        assert_eq!(&info["date"], "2025-11-17");
        assert_eq!(&info["source"], "master");
        assert_eq!(&info["revision"], "4fa6e9938c");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin23");
    }

    #[test]
    fn test_parse_description_yjit() {
        let info = parse_description(
            "ruby 4.0.0 (2025-12-25 revision 553f1675f3) +YJIT +PRISM [arm64-darwin23]",
        )
        .unwrap();
        assert_eq!(&info["version"], "4.0.0");
        assert_eq!(&info["date"], "2025-12-25");
        assert_eq!(&info["source"], "revision");
        assert_eq!(&info["revision"], "553f1675f3");
        assert_eq!(&info["yjit"], "+YJIT ");
        assert_eq!(&info["prism"], "+PRISM ");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin23");
    }

    #[test]
    fn test_parse_description_dev() {
        let info = parse_description(
            "ruby 4.1.0dev (2026-02-12T03:11:10Z master 8f7c12830f) +PRISM [arm64-darwin23]",
        )
        .unwrap();
        assert_eq!(&info["version"], "4.1.0dev");
        assert_eq!(&info["date"], "2026-02-12");
        assert_eq!(&info["source"], "master");
        assert_eq!(&info["revision"], "8f7c12830f");
        assert_eq!(&info["prism"], "+PRISM ");
        assert_eq!(&info["arch"], "arm64");
        assert_eq!(&info["os"], "darwin23");
    }
}
