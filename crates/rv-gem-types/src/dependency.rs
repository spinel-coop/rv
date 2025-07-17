use crate::Requirement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub requirement: Requirement,
    pub dependency_type: DependencyType,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyType {
    Runtime,
    Development,
}

impl Dependency {
    pub fn new(
        name: String,
        requirement: Requirement,
        dependency_type: DependencyType,
        groups: Vec<String>,
    ) -> Self {
        Self {
            name,
            requirement,
            dependency_type,
            groups,
        }
    }
}
