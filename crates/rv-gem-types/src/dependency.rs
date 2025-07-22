use crate::{Requirement, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub requirement: Requirement,
    pub dep_type: DependencyType,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub enum DependencyType {
    #[default]
    Runtime,
    Development,
}

impl AsRef<str> for DependencyType {
    fn as_ref(&self) -> &str {
        match self {
            DependencyType::Runtime => "runtime",
            DependencyType::Development => "development",
        }
    }
}

impl Dependency {
    pub fn new(
        name: String,
        requirements: Vec<String>,
        dep_type: Option<DependencyType>,
    ) -> Result<Self, DependencyError> {
        if name.is_empty() {
            return Err(DependencyError::EmptyName);
        }

        let requirement = Requirement::new(requirements)?;
        let dep_type = dep_type.unwrap_or_default();

        Ok(Self {
            name,
            requirement,
            dep_type,
        })
    }

    pub fn runtime(name: String, requirements: Vec<String>) -> Result<Self, DependencyError> {
        Self::new(name, requirements, Some(DependencyType::Runtime))
    }

    pub fn development(name: String, requirements: Vec<String>) -> Result<Self, DependencyError> {
        Self::new(name, requirements, Some(DependencyType::Development))
    }

    pub fn matches(&self, name: &str, version: &Version, allow_prerelease: bool) -> bool {
        if self.name != name {
            return false;
        }

        // Check prerelease logic
        if version.is_prerelease() && !allow_prerelease && !self.requirement.is_prerelease() {
            return false;
        }

        self.requirement.satisfied_by(version)
    }

    pub fn matches_spec(&self, name: &str, version: &Version) -> bool {
        self.matches(name, version, false)
    }

    pub fn is_runtime(&self) -> bool {
        matches!(self.dep_type, DependencyType::Runtime)
    }

    pub fn is_development(&self) -> bool {
        matches!(self.dep_type, DependencyType::Development)
    }

    pub fn is_latest_version(&self) -> bool {
        // Check if the requirement is just ">= 0"
        self.requirement.constraints.len() == 1
            && matches!(
                self.requirement.constraints[0].operator,
                crate::requirement::ComparisonOperator::GreaterEqual
            )
            && self.requirement.constraints[0].version.to_string() == "0"
    }

    pub fn is_specific(&self) -> bool {
        !self.is_latest_version()
    }

    pub fn merge(&self, other: &Dependency) -> Result<Dependency, DependencyError> {
        if self.name != other.name {
            return Err(DependencyError::NameMismatch {
                name1: self.name.clone(),
                name2: other.name.clone(),
            });
        }

        let mut merged_requirements = self.requirement.constraints.clone();
        merged_requirements.extend(other.requirement.constraints.clone());

        let merged_requirement = Requirement {
            constraints: merged_requirements,
        };

        Ok(Dependency {
            name: self.name.clone(),
            requirement: merged_requirement,
            dep_type: self.dep_type.clone(),
        })
    }

    pub fn requirements_list(&self) -> Vec<String> {
        self.requirement
            .constraints
            .iter()
            .map(|constraint| constraint.to_string())
            .collect()
    }

    pub fn to_lock_name(&self) -> String {
        if self.is_latest_version() {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.requirement)
        }
    }
}

impl std::fmt::Display for Dependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.requirement)
    }
}

impl From<&str> for Dependency {
    fn from(name: &str) -> Self {
        Self::new(name.to_string(), vec![], None).unwrap()
    }
}

impl From<String> for Dependency {
    fn from(name: String) -> Self {
        Self::new(name, vec![], None).unwrap()
    }
}

impl From<(String, Vec<String>)> for Dependency {
    fn from((name, requirements): (String, Vec<String>)) -> Self {
        Self::new(name, requirements, None).unwrap()
    }
}

impl From<(String, Vec<String>, DependencyType)> for Dependency {
    fn from((name, requirements, dep_type): (String, Vec<String>, DependencyType)) -> Self {
        Self::new(name, requirements, Some(dep_type)).unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DependencyError {
    #[error("Dependency name cannot be empty")]
    EmptyName,
    #[error("Cannot merge dependencies with different names: {name1} and {name2}")]
    NameMismatch { name1: String, name2: String },
    #[error("Invalid requirement: {0}")]
    InvalidRequirement(#[from] crate::requirement::RequirementError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_creation() {
        let dep = Dependency::new("test".to_string(), vec!["~> 1.0".to_string()], None).unwrap();
        assert_eq!(dep.name, "test");
        assert!(dep.is_runtime());
        assert!(!dep.is_development());
        assert!(dep.is_specific());
        assert!(!dep.is_latest_version());
    }

    #[test]
    fn test_dependency_types() {
        let runtime_dep = Dependency::runtime("test".to_string(), vec![]).unwrap();
        assert!(runtime_dep.is_runtime());
        assert!(!runtime_dep.is_development());

        let dev_dep = Dependency::development("test".to_string(), vec![]).unwrap();
        assert!(!dev_dep.is_runtime());
        assert!(dev_dep.is_development());
    }

    #[test]
    fn test_dependency_matching() {
        let dep = Dependency::new("test".to_string(), vec![">= 1.0".to_string()], None).unwrap();
        let version_1_0 = Version::new("1.0").unwrap();
        let version_0_9 = Version::new("0.9").unwrap();
        let version_prerelease = Version::new("1.0.alpha").unwrap();
        let version_prerelease_higher = Version::new("1.1.alpha").unwrap();

        assert!(dep.matches("test", &version_1_0, false));
        assert!(!dep.matches("test", &version_0_9, false));
        assert!(!dep.matches("other", &version_1_0, false));
        assert!(!dep.matches("test", &version_prerelease, false));
        assert!(!dep.matches("test", &version_prerelease, true)); // 1.0.alpha < 1.0
        assert!(dep.matches("test", &version_prerelease_higher, true)); // 1.1.alpha >= 1.0
    }

    #[test]
    fn test_dependency_prerelease() {
        let dep =
            Dependency::new("test".to_string(), vec![">= 1.0.alpha".to_string()], None).unwrap();
        let version_prerelease = Version::new("1.0.alpha").unwrap();

        assert!(dep.matches("test", &version_prerelease, false));
    }

    #[test]
    fn test_dependency_merge() {
        let dep1 = Dependency::new("test".to_string(), vec![">= 1.0".to_string()], None).unwrap();
        let dep2 = Dependency::new("test".to_string(), vec!["< 2.0".to_string()], None).unwrap();

        let merged = dep1.merge(&dep2).unwrap();
        assert_eq!(merged.name, "test");
        assert_eq!(merged.requirements_list().len(), 2);
        assert!(merged.requirements_list().contains(&">= 1.0".to_string()));
        assert!(merged.requirements_list().contains(&"< 2.0".to_string()));
    }

    #[test]
    fn test_dependency_merge_different_names() {
        let dep1 = Dependency::new("test1".to_string(), vec![], None).unwrap();
        let dep2 = Dependency::new("test2".to_string(), vec![], None).unwrap();

        let result = dep1.merge(&dep2);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DependencyError::NameMismatch { .. }
        ));
    }

    #[test]
    fn test_dependency_latest_version() {
        let dep = Dependency::new("test".to_string(), vec![], None).unwrap();
        assert!(dep.is_latest_version());
        assert!(!dep.is_specific());
    }

    #[test]
    fn test_dependency_display() {
        let dep = Dependency::new("test".to_string(), vec![">= 1.0".to_string()], None).unwrap();
        assert_eq!(dep.to_string(), "test (>= 1.0)");

        let dep = Dependency::new("test".to_string(), vec![], None).unwrap();
        assert_eq!(dep.to_string(), "test (>= 0)");
    }

    #[test]
    fn test_dependency_to_lock_name() {
        let dep = Dependency::new("test".to_string(), vec![">= 1.0".to_string()], None).unwrap();
        assert_eq!(dep.to_lock_name(), "test (>= 1.0)");

        let dep = Dependency::new("test".to_string(), vec![], None).unwrap();
        assert_eq!(dep.to_lock_name(), "test");
    }

    #[test]
    fn test_dependency_from_conversions() {
        let dep: Dependency = "test".into();
        assert_eq!(dep.name, "test");
        assert!(dep.is_runtime());
        assert!(dep.is_latest_version());

        let dep: Dependency = ("test".to_string(), vec![">= 1.0".to_string()]).into();
        assert_eq!(dep.name, "test");
        assert!(dep.is_specific());

        let dep: Dependency = (
            "test".to_string(),
            vec![">= 1.0".to_string()],
            DependencyType::Development,
        )
            .into();
        assert_eq!(dep.name, "test");
        assert!(dep.is_development());
    }

    #[test]
    fn test_empty_name_error() {
        let result = Dependency::new("".to_string(), vec![], None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DependencyError::EmptyName));
    }
}
