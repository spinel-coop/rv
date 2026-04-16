use crate::{Platform, PlatformError};
use crate::{Version, VersionError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReleaseTuple {
    pub name: String,
    pub version: Version,
    pub platform: Platform,
}

impl ReleaseTuple {
    pub fn new(name: String, version: Version, platform: Option<Platform>) -> Self {
        let platform = platform.unwrap_or(Platform::Ruby);

        Self {
            name,
            version,
            platform,
        }
    }

    pub fn from_array(array: &[String]) -> Result<Self, ReleaseTupleError> {
        let Some(name) = array.first().cloned() else {
            return Err(ReleaseTupleError::InvalidArray);
        };
        let Some(version) = array.get(1).map(Version::new) else {
            return Err(ReleaseTupleError::InvalidArray);
        };
        let version = version?;
        let platform = array.get(2).map(Platform::new).transpose()?;
        Ok(Self::new(name, version, platform))
    }

    pub fn full_name(&self) -> String {
        format!("{}-{}", self.name, self.full_version())
    }

    pub fn full_version(&self) -> String {
        if matches!(self.platform, Platform::Ruby) {
            self.version.to_string()
        } else {
            format!("{}-{}", self.version, self.platform)
        }
    }

    pub fn spec_name(&self) -> String {
        format!("{}.gemspec", self.full_name())
    }

    pub fn to_array(&self) -> [String; 3] {
        [
            self.name.clone(),
            self.version.to_string(),
            self.platform.to_string(),
        ]
    }

    pub fn is_prerelease(&self) -> bool {
        self.version.is_prerelease()
    }
}

impl std::fmt::Display for ReleaseTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

impl Ord for ReleaseTuple {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by name, then version, then platform priority
        match self.name.cmp(&other.name) {
            std::cmp::Ordering::Equal => match self.version.cmp(&other.version) {
                std::cmp::Ordering::Equal => self.platform.cmp(&other.platform),
                other => other,
            },
            other => other,
        }
    }
}

impl PartialOrd for ReleaseTuple {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<(String, Version, Option<Platform>)> for ReleaseTuple {
    fn from((name, version, platform): (String, Version, Option<Platform>)) -> Self {
        Self::new(name, version, platform)
    }
}

impl TryFrom<&[String]> for ReleaseTuple {
    type Error = ReleaseTupleError;

    fn try_from(array: &[String]) -> Result<Self, Self::Error> {
        Self::from_array(array)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReleaseTupleError {
    #[error("Invalid array length for ReleaseTuple")]
    InvalidArray,
    #[error("Invalid version in ReleaseTuple")]
    InvalidVersion(#[from] VersionError),
    #[error("Invalid platform in ReleaseTuple")]
    InvalidPlatform(#[from] PlatformError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_release_tuple_creation() {
        let tuple = ReleaseTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.name, "test");
        assert_eq!(tuple.version, Version::new("1.0").unwrap());
        assert_eq!(tuple.platform, Platform::Ruby);
    }

    #[test]
    fn test_release_tuple_with_platform() {
        let tuple = ReleaseTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some(Platform::new("linux").unwrap()),
        );
        assert_eq!(&tuple.platform.to_string(), "linux");
    }

    #[test]
    fn test_full_name() {
        let tuple = ReleaseTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.full_name(), "test-1.0");

        let tuple = ReleaseTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some(Platform::new("linux").unwrap()),
        );
        assert_eq!(tuple.full_name(), "test-1.0-linux");
    }

    #[test]
    fn test_spec_name() {
        let tuple = ReleaseTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.spec_name(), "test-1.0.gemspec");

        let tuple = ReleaseTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some(Platform::new("linux").unwrap()),
        );
        assert_eq!(tuple.spec_name(), "test-1.0-linux.gemspec");
    }

    #[test]
    fn test_to_array() {
        let tuple = ReleaseTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.to_array(), ["test", "1.0", "ruby"]);

        let tuple = ReleaseTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some(Platform::new("linux").unwrap()),
        );
        assert_eq!(tuple.to_array(), ["test", "1.0", "linux"]);
    }

    #[test]
    fn test_from_array() {
        let array = ["test".to_string(), "1.0".to_string()];
        let tuple = ReleaseTuple::from_array(&array).unwrap();
        assert_eq!(tuple.name, "test");
        assert_eq!(tuple.version, Version::new("1.0").unwrap());
        assert_eq!(tuple.platform, Platform::Ruby);

        let array = ["test".to_string(), "1.0".to_string(), "linux".to_string()];
        let tuple = ReleaseTuple::from_array(&array).unwrap();
        assert_eq!(&tuple.platform.to_string(), "linux");
    }

    #[test]
    fn test_prerelease() {
        let tuple = ReleaseTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert!(!tuple.is_prerelease());

        let tuple = ReleaseTuple::new("test".to_string(), Version::new("1.0.alpha").unwrap(), None);
        assert!(tuple.is_prerelease());
    }

    #[test]
    fn test_sorting() {
        let tuple1 = ReleaseTuple::new("a".to_string(), Version::new("1.0").unwrap(), None);
        let tuple2 = ReleaseTuple::new("b".to_string(), Version::new("1.0").unwrap(), None);
        let tuple3 = ReleaseTuple::new("a".to_string(), Version::new("2.0").unwrap(), None);
        let tuple4 = ReleaseTuple::new(
            "a".to_string(),
            Version::new("1.0").unwrap(),
            Some(Platform::new("linux").unwrap()),
        );

        assert!(tuple1 < tuple2);
        assert!(tuple1 < tuple3);
        assert!(tuple1 < tuple4); // ruby platform has priority
    }

    #[test]
    fn test_display() {
        let tuple = ReleaseTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.to_string(), "test-1.0");

        let tuple = ReleaseTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some(Platform::new("linux").unwrap()),
        );
        assert_eq!(tuple.to_string(), "test-1.0-linux");
    }
}
