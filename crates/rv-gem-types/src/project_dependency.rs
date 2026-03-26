use crate::Requirement;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct ProjectDependency {
    /// What gem this dependency uses.
    pub name: String,
    /// Constraints on what version of the gem can be used.
    pub requirement: Requirement,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectDependencyError {
    #[error("Dependency name cannot be empty")]
    EmptyName,
    #[error("Invalid requirement: {0}")]
    InvalidRequirement(#[from] crate::requirement::RequirementError),
}

impl std::fmt::Debug for ProjectDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.name, self.requirement)
    }
}

impl std::fmt::Display for ProjectDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;

        if !self.requirement.is_latest_version() {
            write!(f, " ({})", self.requirement)?;
        }

        Ok(())
    }
}

impl ProjectDependency {
    pub fn new(name: String, requirements: Vec<String>) -> Result<Self, ProjectDependencyError> {
        if name.is_empty() {
            return Err(ProjectDependencyError::EmptyName);
        }

        let requirement = Requirement::new(requirements)?;

        Ok(Self { name, requirement })
    }
}
