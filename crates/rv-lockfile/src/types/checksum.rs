use crate::types::Platform;
use semver::Version;
use std::fmt;

/// Represents a checksum entry for a gem
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checksum {
    /// The gem name
    pub name: String,
    /// The gem version
    pub version: Version,
    /// The platform (optional)
    pub platform: Option<Platform>,
    /// The checksum algorithm (e.g., "sha256")
    pub algorithm: String,
    /// The checksum value
    pub value: String,
}

impl Checksum {
    /// Create a new checksum
    pub fn new(
        name: String,
        version: Version,
        platform: Option<Platform>,
        algorithm: String,
        value: String,
    ) -> Self {
        Self {
            name,
            version,
            platform,
            algorithm,
            value,
        }
    }

    /// Get the full identifier for this checksum (name-version-platform)
    pub fn full_name(&self) -> String {
        if let Some(ref platform) = self.platform {
            format!("{}-{}-{}", self.name, self.version, platform)
        } else {
            format!("{}-{}", self.name, self.version)
        }
    }

    /// Check if this checksum matches the given algorithm
    pub fn matches_algorithm(&self, algorithm: &str) -> bool {
        self.algorithm == algorithm
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref platform) = self.platform {
            write!(
                f,
                "{} ({}-{}) {}={}",
                self.name, self.version, platform, self.algorithm, self.value
            )
        } else {
            write!(
                f,
                "{} ({}) {}={}",
                self.name, self.version, self.algorithm, self.value
            )
        }
    }
}

impl PartialOrd for Checksum {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Checksum {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by name first, then version, then platform
        self.name
            .cmp(&other.name)
            .then_with(|| self.version.cmp(&other.version))
            .then_with(|| self.platform.cmp(&other.platform))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_creation() {
        let version = Version::parse("1.0.0").unwrap();
        let checksum = Checksum::new(
            "test-gem".to_string(),
            version.clone(),
            None,
            "sha256".to_string(),
            "abc123".to_string(),
        );

        assert_eq!(checksum.name, "test-gem");
        assert_eq!(checksum.version, version);
        assert_eq!(checksum.platform, None);
        assert_eq!(checksum.algorithm, "sha256");
        assert_eq!(checksum.value, "abc123");
    }

    #[test]
    fn test_checksum_with_platform() {
        let version = Version::parse("1.0.0").unwrap();
        let platform = Platform::new("x86_64", "linux", None);
        let checksum = Checksum::new(
            "test-gem".to_string(),
            version,
            Some(platform.clone()),
            "sha256".to_string(),
            "abc123".to_string(),
        );

        assert_eq!(checksum.full_name(), "test-gem-1.0.0-x86_64-linux");
        assert_eq!(checksum.platform, Some(platform));
    }

    #[test]
    fn test_checksum_ordering() {
        let version1 = Version::parse("1.0.0").unwrap();
        let version2 = Version::parse("2.0.0").unwrap();

        let checksum1 = Checksum::new(
            "a-gem".to_string(),
            version1.clone(),
            None,
            "sha256".to_string(),
            "abc123".to_string(),
        );

        let checksum2 = Checksum::new(
            "b-gem".to_string(),
            version1,
            None,
            "sha256".to_string(),
            "def456".to_string(),
        );

        let checksum3 = Checksum::new(
            "a-gem".to_string(),
            version2,
            None,
            "sha256".to_string(),
            "ghi789".to_string(),
        );

        assert!(checksum1 < checksum2); // a-gem < b-gem
        assert!(checksum1 < checksum3); // 1.0.0 < 2.0.0
    }

    #[test]
    fn test_checksum_display() {
        let version = Version::parse("1.0.0").unwrap();
        let checksum = Checksum::new(
            "test-gem".to_string(),
            version,
            None,
            "sha256".to_string(),
            "abc123".to_string(),
        );

        assert_eq!(checksum.to_string(), "test-gem (1.0.0) sha256=abc123");

        let platform = Platform::new("x86_64", "linux", None);
        let checksum_with_platform = Checksum::new(
            "test-gem".to_string(),
            Version::parse("1.0.0").unwrap(),
            Some(platform),
            "sha256".to_string(),
            "abc123".to_string(),
        );

        assert_eq!(
            checksum_with_platform.to_string(),
            "test-gem (1.0.0-x86_64-linux) sha256=abc123"
        );
    }
}