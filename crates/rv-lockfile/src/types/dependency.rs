use crate::types::Platform;
use semver::{Version, VersionReq};

/// Represents a gem dependency with version requirements
#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
    pub name: String,
    pub requirements: Vec<VersionReq>,
    pub platforms: Vec<Platform>,
    pub source: Option<String>, // Source identifier
    pub pinned: bool, // Marked with '!' in lockfile
}

impl Dependency {
    /// Create a new dependency
    pub fn new(name: String) -> Self {
        Dependency {
            name,
            requirements: Vec::new(),
            platforms: Vec::new(),
            source: None,
            pinned: false,
        }
    }
    
    /// Add a version requirement
    pub fn add_requirement(&mut self, req: VersionReq) {
        self.requirements.push(req);
    }
    
    /// Add a platform constraint
    pub fn add_platform(&mut self, platform: Platform) {
        if !self.platforms.contains(&platform) {
            self.platforms.push(platform);
        }
    }
    
    /// Set the source for this dependency
    pub fn set_source(&mut self, source: String) {
        self.source = Some(source);
    }
    
    /// Mark this dependency as pinned
    pub fn set_pinned(&mut self, pinned: bool) {
        self.pinned = pinned;
    }
    
    /// Check if this dependency satisfies a given version
    pub fn satisfies(&self, version: &Version) -> bool {
        if self.requirements.is_empty() {
            return true; // No requirements means any version
        }
        
        self.requirements.iter().all(|req| req.matches(version))
    }
    
    /// Get a string representation of the version requirements
    pub fn requirement_string(&self) -> String {
        if self.requirements.is_empty() {
            ">= 0".to_string()
        } else {
            self.requirements
                .iter()
                .map(|req| req.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
    
    /// Check if this dependency is platform-specific
    pub fn is_platform_specific(&self) -> bool {
        !self.platforms.is_empty()
    }
    
    /// Check if this dependency applies to a given platform
    pub fn applies_to_platform(&self, platform: &Platform) -> bool {
        if self.platforms.is_empty() {
            return true; // No platform constraints means applies to all
        }
        
        self.platforms.contains(platform)
    }
}

impl PartialOrd for Dependency {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dependency {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl Eq for Dependency {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_dependency_creation() {
        let dep = Dependency::new("rails".to_string());
        assert_eq!(dep.name, "rails");
        assert_eq!(dep.requirements.len(), 0);
        assert!(!dep.pinned);
    }
    
    #[test]
    fn test_version_requirements() {
        let mut dep = Dependency::new("rails".to_string());
        dep.add_requirement(VersionReq::parse(">=7.0.0, <8.0.0").unwrap());
        
        let version_7_0_0 = Version::parse("7.0.0").unwrap();
        let version_7_1_0 = Version::parse("7.1.0").unwrap();
        let version_8_0_0 = Version::parse("8.0.0").unwrap();
        
        assert!(dep.satisfies(&version_7_0_0));
        assert!(dep.satisfies(&version_7_1_0));
        assert!(!dep.satisfies(&version_8_0_0));
    }
    
    #[test]
    fn test_platform_constraints() {
        let mut dep = Dependency::new("gem".to_string());
        let ruby_platform = Platform::Ruby;
        let linux_platform = Platform::from_str("x86_64-linux").unwrap();
        
        dep.add_platform(ruby_platform.clone());
        
        assert!(dep.is_platform_specific());
        assert!(dep.applies_to_platform(&ruby_platform));
        assert!(!dep.applies_to_platform(&linux_platform));
    }
    
    #[test]
    fn test_dependency_ordering() {
        let dep1 = Dependency::new("a".to_string());
        let dep2 = Dependency::new("b".to_string());
        let dep3 = Dependency::new("z".to_string());
        
        assert!(dep1 < dep2);
        assert!(dep2 < dep3);
        assert!(dep1 < dep3);
    }
}