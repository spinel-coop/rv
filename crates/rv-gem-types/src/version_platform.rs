use std::str::FromStr;

use rv_version::Version;
use serde::{Deserialize, Serialize};

use crate::Platform;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionPlatform {
    pub version: Version,
    pub platform: Platform,
}

#[derive(Debug, thiserror::Error)]
pub enum VersionPlatformError {
    #[error(transparent)]
    Version(#[from] rv_version::VersionError),
    #[error(transparent)]
    Platform(#[from] crate::platform::PlatformError),
}

/// Splits a gem version with a platform suffix, like `1.11.0.rc1-x86_64-linux`,
/// into its version and platform components.
impl FromStr for VersionPlatform {
    type Err = VersionPlatformError;

    /// Splits a gem version with a platform suffix, like `1.11.0.rc1-x86_64-linux`,
    /// into its version and platform components.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (v, p) = s.split_once('-').unwrap_or((s, "ruby"));

        let version = Version::new(v)?;
        let platform = Platform::new(p)?;

        Ok(VersionPlatform { version, platform })
    }
}

impl std::fmt::Display for VersionPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.platform {
            Platform::Ruby => write!(f, "{}", self.version),
            _ => write!(f, "{}-{}", self.version, self.platform),
        }
    }
}

impl Ord for VersionPlatform {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by version, then platform
        self.version
            .cmp(&other.version)
            .then_with(|| self.platform.cmp(&other.platform))
    }
}

impl PartialOrd for VersionPlatform {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
