use crate::{Dependency, Platform, Requirement, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Specification {
    pub name: String,
    pub version: Version,
    pub platform: Platform,
    pub dependencies: Vec<Dependency>,
    pub authors: Vec<String>,
    pub email: Option<String>,
    pub homepage: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub licenses: Vec<String>,
    pub files: Vec<String>,
    pub executables: Vec<String>,
    pub extensions: Vec<String>,
    pub required_ruby_version: Option<Requirement>,
    pub required_rubygems_version: Option<Requirement>,
}

impl Specification {
    pub fn new(name: String, version: Version, platform: Platform) -> Self {
        Self {
            name,
            version,
            platform,
            dependencies: Vec::new(),
            authors: Vec::new(),
            email: None,
            homepage: None,
            summary: None,
            description: None,
            licenses: Vec::new(),
            files: Vec::new(),
            executables: Vec::new(),
            extensions: Vec::new(),
            required_ruby_version: None,
            required_rubygems_version: None,
        }
    }
}
