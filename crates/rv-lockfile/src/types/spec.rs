use crate::types::{Dependency, Platform};
use semver::Version;
use std::collections::HashMap;

/// Represents a lazy specification, similar to Bundler's LazySpecification
#[derive(Debug, Clone, PartialEq)]
pub struct LazySpecification {
    pub name: String,
    pub version: Version,
    pub platform: Platform,
    pub source: Option<String>, // Source identifier
    pub dependencies: Vec<Dependency>,
    pub checksum: Option<String>,
    pub required_ruby_version: Option<String>,
    pub required_rubygems_version: Option<String>,
}

impl LazySpecification {
    /// Create a new lazy specification
    pub fn new(name: String, version: Version, platform: Platform) -> Self {
        LazySpecification {
            name,
            version,
            platform,
            source: None,
            dependencies: Vec::new(),
            checksum: None,
            required_ruby_version: None,
            required_rubygems_version: None,
        }
    }
    
    /// Get the full name including version and platform
    pub fn full_name(&self) -> String {
        if self.platform.is_ruby() {
            format!("{} ({})", self.name, self.version)
        } else {
            format!("{} ({}-{})", self.name, self.version, self.platform)
        }
    }
    
    /// Get the name and version only
    pub fn name_version(&self) -> String {
        format!("{} ({})", self.name, self.version)
    }
    
    /// Add a dependency to this specification
    pub fn add_dependency(&mut self, dependency: Dependency) {
        self.dependencies.push(dependency);
    }
    
    /// Set the source for this specification
    pub fn set_source(&mut self, source: String) {
        self.source = Some(source);
    }
    
    /// Set the checksum for this specification
    pub fn set_checksum(&mut self, checksum: String) {
        self.checksum = Some(checksum);
    }
    
    /// Set the required Ruby version
    pub fn set_required_ruby_version(&mut self, version: String) {
        self.required_ruby_version = Some(version);
    }
    
    /// Set the required RubyGems version  
    pub fn set_required_rubygems_version(&mut self, version: String) {
        self.required_rubygems_version = Some(version);
    }
    
    /// Check if this spec has a specific platform (not ruby)
    pub fn is_platform_specific(&self) -> bool {
        !self.platform.is_ruby()
    }
    
    /// Get dependencies that apply to a specific platform
    pub fn dependencies_for_platform(&self, platform: &Platform) -> Vec<&Dependency> {
        self.dependencies
            .iter()
            .filter(|dep| dep.applies_to_platform(platform))
            .collect()
    }
    
    /// Check if this spec has runtime dependencies
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }
    
    /// Get the source type if available
    pub fn source_type(&self) -> Option<&str> {
        self.source.as_deref()
    }
}

impl PartialOrd for LazySpecification {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LazySpecification {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by name first, then by version (descending), then by platform
        match self.name.cmp(&other.name) {
            std::cmp::Ordering::Equal => {
                match other.version.cmp(&self.version) { // Reverse version order
                    std::cmp::Ordering::Equal => self.platform.cmp(&other.platform),
                    other => other,
                }
            }
            other => other,
        }
    }
}

impl Eq for LazySpecification {}

/// Collection of specifications organized by full name
#[derive(Debug, Clone)]
pub struct SpecificationSet {
    specs: HashMap<String, LazySpecification>,
}

impl SpecificationSet {
    /// Create a new empty specification set
    pub fn new() -> Self {
        SpecificationSet {
            specs: HashMap::new(),
        }
    }
    
    /// Add a specification to the set
    pub fn add(&mut self, spec: LazySpecification) {
        let full_name = spec.full_name();
        self.specs.insert(full_name, spec);
    }
    
    /// Get a specification by full name
    pub fn get(&self, full_name: &str) -> Option<&LazySpecification> {
        self.specs.get(full_name)
    }
    
    /// Get all specifications
    pub fn all(&self) -> Vec<&LazySpecification> {
        self.specs.values().collect()
    }
    
    /// Get specifications for a specific platform
    pub fn for_platform(&self, platform: &Platform) -> Vec<&LazySpecification> {
        self.specs
            .values()
            .filter(|spec| spec.platform == *platform || spec.platform.is_ruby())
            .collect()
    }
    
    /// Find specifications by name (all versions/platforms)
    pub fn find_by_name(&self, name: &str) -> Vec<&LazySpecification> {
        self.specs
            .values()
            .filter(|spec| spec.name == name)
            .collect()
    }
    
    /// Get the number of specifications
    pub fn len(&self) -> usize {
        self.specs.len()
    }
    
    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }
}

impl Default for SpecificationSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_lazy_specification() {
        let version = Version::parse("1.0.0").unwrap();
        let platform = Platform::Ruby;
        let spec = LazySpecification::new("test-gem".to_string(), version, platform);
        
        assert_eq!(spec.name, "test-gem");
        assert_eq!(spec.version.to_string(), "1.0.0");
        assert_eq!(spec.full_name(), "test-gem (1.0.0)");
        assert!(!spec.is_platform_specific());
    }
    
    #[test]
    fn test_platform_specific_spec() {
        let version = Version::parse("1.0.0").unwrap();
        let platform = Platform::from_str("x86_64-linux").unwrap();
        let spec = LazySpecification::new("platform-gem".to_string(), version, platform.clone());
        
        assert_eq!(spec.full_name(), "platform-gem (1.0.0-x86_64-linux)");
        assert!(spec.is_platform_specific());
        assert_eq!(spec.platform, platform);
    }
    
    #[test]
    fn test_specification_set() {
        let mut set = SpecificationSet::new();
        
        let spec1 = LazySpecification::new(
            "gem1".to_string(),
            Version::parse("1.0.0").unwrap(),
            Platform::Ruby,
        );
        let spec2 = LazySpecification::new(
            "gem2".to_string(),
            Version::parse("2.0.0").unwrap(),
            Platform::from_str("x86_64-linux").unwrap(),
        );
        
        set.add(spec1.clone());
        set.add(spec2.clone());
        
        assert_eq!(set.len(), 2);
        assert!(set.get(&spec1.full_name()).is_some());
        assert!(set.get(&spec2.full_name()).is_some());
        
        let ruby_specs = set.for_platform(&Platform::Ruby);
        assert_eq!(ruby_specs.len(), 1); // Only spec1 is for ruby platform
    }
    
    #[test]
    fn test_specification_ordering() {
        let spec1 = LazySpecification::new(
            "a".to_string(),
            Version::parse("1.0.0").unwrap(),
            Platform::Ruby,
        );
        let spec2 = LazySpecification::new(
            "a".to_string(),
            Version::parse("2.0.0").unwrap(),
            Platform::Ruby,
        );
        let spec3 = LazySpecification::new(
            "b".to_string(),
            Version::parse("1.0.0").unwrap(),
            Platform::Ruby,
        );
        
        // Same name: higher version comes first
        assert!(spec2 < spec1);
        // Different names: alphabetical order
        assert!(spec1 < spec3);
    }
}