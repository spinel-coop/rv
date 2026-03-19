use crate::Requirement;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDependency {
    /// What gem this dependency uses.
    pub name: String,
    /// Constraints on what version of the gem can be used.
    pub requirement: Requirement,
}

impl std::fmt::Debug for ProjectDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.name, self.requirement)
    }
}
