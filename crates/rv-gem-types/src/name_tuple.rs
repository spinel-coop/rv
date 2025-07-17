use crate::{Platform, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameTuple {
    pub name: String,
    pub version: Version,
    pub platform: Platform,
}

impl NameTuple {
    pub fn new(name: String, version: Version, platform: Platform) -> Self {
        Self {
            name,
            version,
            platform,
        }
    }
}

impl std::fmt::Display for NameTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", self.name, self.version, self.platform)
    }
}
