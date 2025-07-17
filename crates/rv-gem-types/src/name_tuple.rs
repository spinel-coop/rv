use crate::Version;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NameTuple {
    pub name: String,
    pub version: Version,
    pub platform: String,
}

impl NameTuple {
    pub fn new(name: String, version: Version, platform: Option<String>) -> Self {
        let platform = platform
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "ruby".to_string());
        
        Self { name, version, platform }
    }
    
    pub fn from_array(array: &[String]) -> Result<Self, NameTupleError> {
        if array.len() < 2 || array.len() > 3 {
            return Err(NameTupleError::InvalidArray);
        }
        
        let name = array[0].clone();
        let version = Version::new(&array[1]).map_err(|_| NameTupleError::InvalidVersion)?;
        let platform = if array.len() == 3 {
            Some(array[2].clone())
        } else {
            None
        };
        
        Ok(Self::new(name, version, platform))
    }
    
    pub fn null() -> Self {
        Self {
            name: String::new(),
            version: Version::new("0").unwrap(),
            platform: String::new(),
        }
    }
    
    pub fn full_name(&self) -> String {
        if self.platform == "ruby" || self.platform.is_empty() {
            format!("{}-{}", self.name, self.version)
        } else {
            format!("{}-{}-{}", self.name, self.version, self.platform)
        }
    }
    
    pub fn spec_name(&self) -> String {
        format!("{}.gemspec", self.full_name())
    }
    
    pub fn to_array(&self) -> [String; 3] {
        [self.name.clone(), self.version.to_string(), self.platform.clone()]
    }
    
    pub fn prerelease(&self) -> bool {
        self.version.is_prerelease()
    }
    
    pub fn match_platform(&self, platform: &str) -> bool {
        if self.platform == "ruby" || platform == "ruby" {
            true
        } else {
            self.platform == platform
        }
    }
}

impl std::fmt::Display for NameTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

impl Ord for NameTuple {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by name, then version, then platform priority
        match self.name.cmp(&other.name) {
            std::cmp::Ordering::Equal => {
                match self.version.cmp(&other.version) {
                    std::cmp::Ordering::Equal => {
                        // Ruby platform has priority -1, others have priority 1
                        let self_priority = if self.platform == "ruby" { -1 } else { 1 };
                        let other_priority = if other.platform == "ruby" { -1 } else { 1 };
                        match self_priority.cmp(&other_priority) {
                            std::cmp::Ordering::Equal => self.platform.cmp(&other.platform),
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

impl PartialOrd for NameTuple {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl From<(String, Version, Option<String>)> for NameTuple {
    fn from((name, version, platform): (String, Version, Option<String>)) -> Self {
        Self::new(name, version, platform)
    }
}

impl From<&[String]> for NameTuple {
    fn from(array: &[String]) -> Self {
        Self::from_array(array).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameTupleError {
    #[error("Invalid array length for NameTuple")]
    InvalidArray,
    #[error("Invalid version in NameTuple")]
    InvalidVersion,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_tuple_creation() {
        let tuple = NameTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.name, "test");
        assert_eq!(tuple.version, Version::new("1.0").unwrap());
        assert_eq!(tuple.platform, "ruby");
    }

    #[test]
    fn test_name_tuple_with_platform() {
        let tuple = NameTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some("linux".to_string()),
        );
        assert_eq!(tuple.platform, "linux");
    }

    #[test]
    fn test_full_name() {
        let tuple = NameTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.full_name(), "test-1.0");

        let tuple = NameTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some("linux".to_string()),
        );
        assert_eq!(tuple.full_name(), "test-1.0-linux");
    }

    #[test]
    fn test_spec_name() {
        let tuple = NameTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.spec_name(), "test-1.0.gemspec");

        let tuple = NameTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some("linux".to_string()),
        );
        assert_eq!(tuple.spec_name(), "test-1.0-linux.gemspec");
    }

    #[test]
    fn test_to_array() {
        let tuple = NameTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.to_array(), ["test", "1.0", "ruby"]);

        let tuple = NameTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some("linux".to_string()),
        );
        assert_eq!(tuple.to_array(), ["test", "1.0", "linux"]);
    }

    #[test]
    fn test_from_array() {
        let array = ["test".to_string(), "1.0".to_string()];
        let tuple = NameTuple::from_array(&array).unwrap();
        assert_eq!(tuple.name, "test");
        assert_eq!(tuple.version, Version::new("1.0").unwrap());
        assert_eq!(tuple.platform, "ruby");

        let array = ["test".to_string(), "1.0".to_string(), "linux".to_string()];
        let tuple = NameTuple::from_array(&array).unwrap();
        assert_eq!(tuple.platform, "linux");
    }

    #[test]
    fn test_prerelease() {
        let tuple = NameTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert!(!tuple.prerelease());

        let tuple = NameTuple::new("test".to_string(), Version::new("1.0.alpha").unwrap(), None);
        assert!(tuple.prerelease());
    }

    #[test]
    fn test_match_platform() {
        let tuple = NameTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert!(tuple.match_platform("ruby"));
        assert!(tuple.match_platform("linux"));

        let tuple = NameTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some("linux".to_string()),
        );
        assert!(tuple.match_platform("ruby"));
        assert!(tuple.match_platform("linux"));
        assert!(!tuple.match_platform("windows"));
    }

    #[test]
    fn test_sorting() {
        let tuple1 = NameTuple::new("a".to_string(), Version::new("1.0").unwrap(), None);
        let tuple2 = NameTuple::new("b".to_string(), Version::new("1.0").unwrap(), None);
        let tuple3 = NameTuple::new("a".to_string(), Version::new("2.0").unwrap(), None);
        let tuple4 = NameTuple::new(
            "a".to_string(),
            Version::new("1.0").unwrap(),
            Some("linux".to_string()),
        );

        assert!(tuple1 < tuple2);
        assert!(tuple1 < tuple3);
        assert!(tuple1 < tuple4); // ruby platform has priority
    }

    #[test]
    fn test_display() {
        let tuple = NameTuple::new("test".to_string(), Version::new("1.0").unwrap(), None);
        assert_eq!(tuple.to_string(), "test-1.0");

        let tuple = NameTuple::new(
            "test".to_string(),
            Version::new("1.0").unwrap(),
            Some("linux".to_string()),
        );
        assert_eq!(tuple.to_string(), "test-1.0-linux");
    }

    #[test]
    fn test_null() {
        let tuple = NameTuple::null();
        assert_eq!(tuple.name, "");
        assert_eq!(tuple.version, Version::new("0").unwrap());
        assert_eq!(tuple.platform, "");
    }
}