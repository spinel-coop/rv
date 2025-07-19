use crate::{Dependency, DependencyType, Platform, Requirement, Version};
use indexmap::IndexMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Specification {
    // Required fields
    pub name: String,
    pub version: Version,
    pub summary: String,
    pub require_paths: Vec<String>,
    pub rubygems_version: String,
    pub specification_version: i32,
    pub date: String,

    // Optional fields with defaults
    pub authors: Vec<String>,
    pub email: Vec<String>,
    pub homepage: Option<String>,
    pub description: Option<String>,
    pub licenses: Vec<String>,
    pub files: Vec<String>,
    pub executables: Vec<String>,
    pub extensions: Vec<String>,
    pub dependencies: Vec<Dependency>,
    pub metadata: IndexMap<String, String>,
    pub platform: Platform,
    pub bindir: String,
    pub post_install_message: Option<String>,
    pub requirements: Vec<String>,
    pub required_ruby_version: Requirement,
    pub required_rubygems_version: Requirement,
    pub test_files: Vec<String>,
    pub extra_rdoc_files: Vec<String>,
    pub rdoc_options: Vec<String>,
    pub cert_chain: Vec<String>,
    pub signing_key: Option<String>,
    pub autorequire: Option<String>,
    pub installed_by_version: Option<Version>,
}

impl Specification {
    pub fn new(name: String, version: Version) -> Result<Self, SpecificationError> {
        if name.is_empty() {
            return Err(SpecificationError::EmptyName);
        }

        Ok(Self {
            name,
            version,
            summary: String::new(),
            require_paths: vec!["lib".to_string()],
            rubygems_version: "3.0.0".to_string(),
            specification_version: 4,
            authors: Vec::new(),
            email: Vec::new(),
            homepage: None,
            description: None,
            licenses: Vec::new(),
            files: Vec::new(),
            executables: Vec::new(),
            extensions: Vec::new(),
            dependencies: Vec::new(),
            metadata: IndexMap::new(),
            platform: Platform::Ruby,
            bindir: "bin".to_string(),
            post_install_message: None,
            requirements: Vec::new(),
            required_ruby_version: Requirement::default(),
            required_rubygems_version: Requirement::default(),
            test_files: Vec::new(),
            extra_rdoc_files: Vec::new(),
            rdoc_options: Vec::new(),
            cert_chain: Vec::new(),
            signing_key: None,
            autorequire: None,
            date: "".to_string(),
            installed_by_version: None,
        })
    }

    pub fn add_dependency(
        &mut self,
        name: String,
        requirements: Vec<String>,
    ) -> Result<(), SpecificationError> {
        let dependency = Dependency::new(name, requirements, Some(DependencyType::Runtime))?;
        self.dependencies.push(dependency);
        Ok(())
    }

    pub fn add_development_dependency(
        &mut self,
        name: String,
        requirements: Vec<String>,
    ) -> Result<(), SpecificationError> {
        let dependency = Dependency::new(name, requirements, Some(DependencyType::Development))?;
        self.dependencies.push(dependency);
        Ok(())
    }

    pub fn runtime_dependencies(&self) -> Vec<&Dependency> {
        self.dependencies
            .iter()
            .filter(|d| d.is_runtime())
            .collect()
    }

    pub fn development_dependencies(&self) -> Vec<&Dependency> {
        self.dependencies
            .iter()
            .filter(|d| d.is_development())
            .collect()
    }

    pub fn satisfies_requirement(&self, dependency: &Dependency) -> bool {
        self.name == dependency.name && dependency.requirement.satisfied_by(&self.version)
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate required fields
        if self.name.is_empty() {
            errors.push("name is required".to_string());
        }

        if self.summary.is_empty() {
            errors.push("summary is required".to_string());
        }

        if self.require_paths.is_empty() {
            errors.push("require_paths cannot be empty".to_string());
        }

        // Validate name format (alphanumeric, dots, dashes, underscores)
        if !self
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || ".-_".contains(c))
        {
            errors.push("name contains invalid characters".to_string());
        }

        // Validate metadata
        for (key, value) in &self.metadata {
            if key.len() > 128 {
                errors.push(format!("metadata key '{key}' is too long (max 128 bytes)"));
            }
            if value.len() > 1024 {
                errors.push(format!(
                    "metadata value for '{key}' is too long (max 1024 bytes)"
                ));
            }
        }

        // Validate no duplicate dependencies
        let mut dep_names = std::collections::HashSet::new();
        for dep in &self.dependencies {
            let dep_key = (&dep.name, &dep.dep_type);
            if dep_names.contains(&dep_key) {
                errors.push(format!(
                    "duplicate {} dependency: {}",
                    match dep.dep_type {
                        DependencyType::Runtime => "runtime",
                        DependencyType::Development => "development",
                    },
                    dep.name
                ));
            }
            dep_names.insert(dep_key);
        }

        // Validate licenses
        if !self.licenses.is_empty() {
            for license in &self.licenses {
                if license.is_empty() {
                    errors.push("license cannot be empty".to_string());
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn to_ruby(&self) -> String {
        let mut lines = vec![
            "# -*- encoding: utf-8 -*-".to_string(),
            "".to_string(),
            "Gem::Specification.new do |s|".to_string(),
        ];

        // Required fields
        lines.push(format!("  s.name = {:?}", self.name));
        lines.push(format!("  s.version = \"{}\"", self.version));
        lines.push(format!("  s.summary = {:?}", self.summary));

        // Authors
        if !self.authors.is_empty() {
            lines.push(format!("  s.authors = {:?}", self.authors));
        }

        // Email
        if !self.email.is_empty() {
            lines.push(format!("  s.email = {:?}", self.email));
        }

        // Description
        if let Some(description) = &self.description {
            lines.push(format!("  s.description = {description:?}"));
        }

        // Homepage
        if let Some(homepage) = &self.homepage {
            lines.push(format!("  s.homepage = {homepage:?}"));
        }

        // Licenses
        if !self.licenses.is_empty() {
            if self.licenses.len() == 1 {
                lines.push(format!("  s.license = {:?}", self.licenses[0]));
            } else {
                lines.push(format!("  s.licenses = {:?}", self.licenses));
            }
        }

        // Files
        if !self.files.is_empty() {
            lines.push(format!("  s.files = {:?}", self.files));
        }

        // Executables
        if !self.executables.is_empty() {
            lines.push(format!("  s.executables = {:?}", self.executables));
        }

        // Extensions
        if !self.extensions.is_empty() {
            lines.push(format!("  s.extensions = {:?}", self.extensions));
        }

        // Require paths
        if self.require_paths != vec!["lib".to_string()] {
            lines.push(format!("  s.require_paths = {:?}", self.require_paths));
        }

        // Platform
        if !self.platform.is_ruby() {
            lines.push(format!("  s.platform = {:?}", self.platform.to_string()));
        }

        // Bindir
        if self.bindir != "bin" {
            lines.push(format!("  s.bindir = {:?}", self.bindir));
        }

        // Required Ruby version
        if !self.required_ruby_version.is_latest_version() {
            lines.push(format!(
                "  s.required_ruby_version = {:?}",
                self.required_ruby_version.to_string()
            ));
        }

        // Required RubyGems version
        if !self.required_rubygems_version.is_latest_version() {
            lines.push(format!(
                "  s.required_rubygems_version = {:?}",
                self.required_rubygems_version.to_string()
            ));
        }

        // Metadata
        if !self.metadata.is_empty() {
            lines.push("  s.metadata = {".to_string());
            let mut sorted_metadata: Vec<_> = self.metadata.iter().collect();
            sorted_metadata.sort_by_key(|(key, _)| *key);
            for (key, value) in sorted_metadata {
                lines.push(format!("    {key:?} => {value:?},"));
            }
            lines.push("  }".to_string());
        }

        // Post install message
        if let Some(message) = &self.post_install_message {
            lines.push(format!("  s.post_install_message = {message:?}"));
        }

        // Add dependencies
        for dep in &self.dependencies {
            use std::fmt::Write;
            let mut line = "  s.add".to_string();
            match dep.dep_type {
                DependencyType::Development => {
                    line.push_str("_development");
                }
                DependencyType::Runtime => {}
            }
            write!(line, "_dependency {:?}", dep.name).unwrap();
            for req in dep.requirements_list() {
                write!(line, ", {req:?}").unwrap();
            }
            lines.push(line);
        }

        lines.push("end".to_string());
        lines.join("\n")
    }

    pub fn full_name(&self) -> String {
        if self.platform.is_ruby() {
            format!("{}-{}", self.name, self.version)
        } else {
            format!("{}-{}-{}", self.name, self.version, self.platform)
        }
    }

    pub fn is_prerelease(&self) -> bool {
        self.version.is_prerelease()
    }

    pub fn has_extensions(&self) -> bool {
        !self.extensions.is_empty()
    }

    pub fn executable_names(&self) -> Vec<String> {
        self.executables.clone()
    }

    pub fn with_summary(mut self, summary: String) -> Self {
        self.summary = summary;
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_authors(mut self, authors: Vec<String>) -> Self {
        self.authors = authors;
        self
    }

    pub fn with_email(mut self, email: Vec<String>) -> Self {
        self.email = email;
        self
    }

    pub fn with_homepage(mut self, homepage: String) -> Self {
        self.homepage = Some(homepage);
        self
    }

    pub fn with_license(mut self, license: String) -> Self {
        self.licenses = vec![license];
        self
    }

    pub fn with_licenses(mut self, licenses: Vec<String>) -> Self {
        self.licenses = licenses;
        self
    }

    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files = files;
        self
    }

    pub fn with_executables(mut self, executables: Vec<String>) -> Self {
        self.executables = executables;
        self
    }

    pub fn with_platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }
}

impl std::fmt::Display for Specification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpecificationError {
    #[error("Specification name cannot be empty")]
    EmptyName,
    #[error("Dependency error: {0}")]
    DependencyError(#[from] crate::dependency::DependencyError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specification_creation() {
        let spec = Specification::new("test".to_string(), Version::new("1.0.0").unwrap()).unwrap();
        assert_eq!(spec.name, "test");
        assert_eq!(spec.version, Version::new("1.0.0").unwrap());
        assert_eq!(spec.platform, Platform::Ruby);
        assert_eq!(spec.require_paths, vec!["lib"]);
        assert_eq!(spec.specification_version, 4);
        assert_eq!(spec.rubygems_version, "3.0.0");
        assert!(spec.summary.is_empty());
        assert!(spec.dependencies.is_empty());
    }

    #[test]
    fn test_specification_with_builder_methods() {
        let spec = Specification::new("test".to_string(), Version::new("1.0.0").unwrap())
            .unwrap()
            .with_summary("A test gem".to_string())
            .with_description("A longer description".to_string())
            .with_authors(vec!["Test Author".to_string()])
            .with_email(vec!["test@example.com".into()])
            .with_homepage("https://example.com".to_string())
            .with_license("MIT".to_string());

        assert_eq!(spec.summary, "A test gem");
        assert_eq!(spec.description, Some("A longer description".to_string()));
        assert_eq!(spec.authors, vec!["Test Author"]);
        assert_eq!(spec.email, vec!["test@example.com"]);
        assert_eq!(spec.homepage, Some("https://example.com".to_string()));
        assert_eq!(spec.licenses, vec!["MIT"]);
    }

    #[test]
    fn test_add_dependencies() {
        let mut spec =
            Specification::new("test".to_string(), Version::new("1.0.0").unwrap()).unwrap();

        spec.add_dependency("runtime_dep".to_string(), vec!["~> 1.0".to_string()])
            .unwrap();
        spec.add_development_dependency("dev_dep".to_string(), vec![">= 0.1".to_string()])
            .unwrap();

        assert_eq!(spec.dependencies.len(), 2);
        assert_eq!(spec.runtime_dependencies().len(), 1);
        assert_eq!(spec.development_dependencies().len(), 1);

        let runtime_dep = &spec.runtime_dependencies()[0];
        assert_eq!(runtime_dep.name, "runtime_dep");
        assert!(runtime_dep.is_runtime());

        let dev_dep = &spec.development_dependencies()[0];
        assert_eq!(dev_dep.name, "dev_dep");
        assert!(dev_dep.is_development());
    }

    #[test]
    fn test_satisfies_requirement() {
        let spec = Specification::new("test".to_string(), Version::new("1.5.0").unwrap()).unwrap();

        let dep1 = Dependency::new("test".to_string(), vec!["~> 1.0".to_string()], None).unwrap();
        let dep2 = Dependency::new("test".to_string(), vec![">= 2.0".to_string()], None).unwrap();
        let dep3 = Dependency::new("other".to_string(), vec!["~> 1.0".to_string()], None).unwrap();

        assert!(spec.satisfies_requirement(&dep1));
        assert!(!spec.satisfies_requirement(&dep2));
        assert!(!spec.satisfies_requirement(&dep3));
    }

    #[test]
    fn test_validation() {
        let mut spec =
            Specification::new("test".to_string(), Version::new("1.0.0").unwrap()).unwrap();

        // Empty summary should fail validation
        let result = spec.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains(&"summary is required".to_string()));

        // With summary should pass
        spec.summary = "Test summary".to_string();
        assert!(spec.validate().is_ok());

        // Invalid name should fail
        spec.name = "invalid name with spaces".to_string();
        let result = spec.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains(&"name contains invalid characters".to_string()));

        // Long metadata should fail
        spec.name = "test".to_string();
        spec.metadata.insert("x".repeat(129), "value".to_string());
        let result = spec.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .iter()
            .any(|e| e.contains("metadata key") && e.contains("too long")));
    }

    #[test]
    fn test_full_name() {
        let spec = Specification::new("test".to_string(), Version::new("1.0.0").unwrap()).unwrap();
        assert_eq!(spec.full_name(), "test-1.0.0");

        let spec = spec.with_platform("x86_64-linux".parse().unwrap());
        assert_eq!(spec.full_name(), "test-1.0.0-x86_64-linux");
    }

    #[test]
    fn test_is_prerelease() {
        let spec = Specification::new("test".to_string(), Version::new("1.0.0").unwrap()).unwrap();
        assert!(!spec.is_prerelease());

        let spec =
            Specification::new("test".to_string(), Version::new("1.0.0.alpha").unwrap()).unwrap();
        assert!(spec.is_prerelease());
    }

    #[test]
    fn test_to_ruby_minimal() {
        let spec = Specification::new("test".to_string(), Version::new("1.0.0").unwrap())
            .unwrap()
            .with_summary("A minimal test gem".to_string());

        insta::assert_snapshot!(spec.to_ruby());
    }

    #[test]
    fn test_to_ruby_comprehensive() {
        let mut spec = Specification::new(
            "comprehensive-gem".to_string(),
            Version::new("2.1.0").unwrap(),
        )
        .unwrap()
        .with_summary("A comprehensive test gem".to_string())
        .with_description("This is a longer description of the test gem".to_string())
        .with_authors(vec!["Test Author".to_string(), "Second Author".to_string()])
        .with_email(vec!["test@example.com".into()])
        .with_homepage("https://example.com".to_string())
        .with_licenses(vec!["MIT".to_string(), "Apache-2.0".to_string()])
        .with_files(vec!["lib/test.rb".to_string(), "README.md".to_string()])
        .with_executables(vec!["test-cli".to_string()])
        .with_platform("x86_64-linux".parse().unwrap());

        spec.add_dependency("runtime_dep".to_string(), vec!["~> 1.0".to_string()])
            .unwrap();
        spec.add_dependency(
            "another_dep".to_string(),
            vec![">= 2.0".to_string(), "< 3.0".to_string()],
        )
        .unwrap();
        spec.add_development_dependency("test_dep".to_string(), vec![">= 0.1".to_string()])
            .unwrap();

        spec.metadata.insert(
            "changelog_uri".to_string(),
            "https://example.com/changelog".to_string(),
        );
        spec.metadata.insert(
            "bug_tracker_uri".to_string(),
            "https://example.com/issues".to_string(),
        );

        spec.post_install_message = Some("Thanks for installing!".to_string());
        spec.extensions = vec!["ext/extconf.rb".to_string()];

        insta::assert_snapshot!(spec.to_ruby());
    }

    #[test]
    fn test_to_ruby_with_custom_requirements() {
        let mut spec = Specification::new(
            "custom-requirements".to_string(),
            Version::new("1.0.0").unwrap(),
        )
        .unwrap()
        .with_summary("Test custom requirements".to_string());

        spec.required_ruby_version = Requirement::new(vec![">= 2.7"]).unwrap();
        spec.required_rubygems_version = Requirement::new(vec![">= 3.0"]).unwrap();
        spec.bindir = "exe".to_string();
        spec.require_paths = vec!["lib".to_string(), "ext".to_string()];

        insta::assert_snapshot!(spec.to_ruby());
    }

    #[test]
    fn test_display() {
        let spec = Specification::new("test".to_string(), Version::new("1.0.0").unwrap()).unwrap();
        assert_eq!(spec.to_string(), "test-1.0.0");
    }

    #[test]
    fn test_empty_name_error() {
        let result = Specification::new("".to_string(), Version::new("1.0.0").unwrap());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SpecificationError::EmptyName));
    }

    #[test]
    fn test_has_extensions() {
        let spec = Specification::new("test".to_string(), Version::new("1.0.0").unwrap()).unwrap();
        assert!(!spec.has_extensions());

        let spec = spec.with_files(vec!["ext/extconf.rb".to_string()]);
        let mut spec = spec;
        spec.extensions = vec!["ext/extconf.rb".to_string()];
        assert!(spec.has_extensions());
    }
}
